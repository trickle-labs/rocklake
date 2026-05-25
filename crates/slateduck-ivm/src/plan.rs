//! View SQL → IVM plan: parse the SELECT statement and extract GROUP BY
//! columns, aggregate functions, and JOIN clauses.
//!
//! Supported SQL subset:
//!   - GROUP BY on one or more named columns
//!   - COUNT(*), SUM(col), AVG(col), MIN(col), MAX(col), STDDEV(col),
//!     BOOL_AND(col), BOOL_OR(col), BIT_AND(col), BIT_OR(col), BIT_XOR(col),
//!     STRING_AGG(col), ARRAY_AGG(col) aggregates
//!   - Single or multi-input JOINs with an equality predicate
//!   - EXPLAIN MATERIALIZED VIEW: returns the selected join strategy per operator
//!   - Volatility validation at compile time (v0.14)

use crate::join::{JoinClause, JoinStrategy, DEFAULT_BROADCAST_THRESHOLD};
use crate::volatility::{self, Volatility};
use crate::worker::IvmError;

/// A single aggregate function in the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregate {
    /// Output column name (alias or auto-generated).
    pub output_col: String,
    /// Aggregate kind.
    pub kind: AggregateKind,
    /// Input column (None for COUNT(*)).
    pub input_col: Option<String>,
}

/// Aggregate tier classification (v0.14).
///
/// Determines how the IVM engine maintains incremental state for this aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateTier {
    /// Fully invertible: no source rescan needed.
    /// COUNT, SUM, AVG, STDDEV, VAR, CORR, REGR_*
    Algebraic,
    /// Partially invertible: rescan group on delete of current extremum/deciding input.
    /// MIN, MAX, BOOL_AND, BOOL_OR, BIT_AND, BIT_OR, BIT_XOR
    SemiAlgebraic,
    /// Not invertible: re-aggregate entire group on each delta.
    /// STRING_AGG, ARRAY_AGG, JSON_AGG, MODE, PERCENTILE_*
    GroupRescan,
}

/// Supported aggregate functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregateKind {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Stddev,
    BoolAnd,
    BoolOr,
    BitAnd,
    BitOr,
    BitXor,
    StringAgg,
    ArrayAgg,
}

impl AggregateKind {
    /// Return the tier classification for this aggregate.
    pub fn tier(&self) -> AggregateTier {
        match self {
            AggregateKind::Count
            | AggregateKind::Sum
            | AggregateKind::Avg
            | AggregateKind::Stddev => AggregateTier::Algebraic,

            AggregateKind::Min
            | AggregateKind::Max
            | AggregateKind::BoolAnd
            | AggregateKind::BoolOr
            | AggregateKind::BitAnd
            | AggregateKind::BitOr
            | AggregateKind::BitXor => AggregateTier::SemiAlgebraic,

            AggregateKind::StringAgg | AggregateKind::ArrayAgg => AggregateTier::GroupRescan,
        }
    }
}

/// Parsed IVM plan extracted from view SQL.
#[derive(Debug, Clone)]
pub struct IvmPlan {
    /// The original view SQL.
    pub view_sql: String,
    /// GROUP BY column names (in order).
    pub group_by_cols: Vec<String>,
    /// Aggregate functions.
    pub aggregates: Vec<Aggregate>,
    /// JOIN clauses (empty for single-table views).
    pub joins: Vec<JoinClause>,
    /// All input table names (may include the FROM table and JOIN targets).
    pub input_tables: Vec<String>,
    /// Broadcast threshold override (0 = use default).
    pub broadcast_threshold: u64,
}

impl IvmPlan {
    /// Parse a view SQL string and extract the IVM plan.
    pub fn parse(view_sql: &str) -> Result<Self, IvmError> {
        use sqlparser::ast::{
            Expr, FunctionArg, FunctionArgExpr, FunctionArguments, GroupByExpr, Join,
            JoinConstraint, JoinOperator as SqlJoinOp, SelectItem, SetExpr, TableFactor,
        };
        use sqlparser::dialect::GenericDialect;
        use sqlparser::parser::Parser;

        let dialect = GenericDialect {};
        let ast = Parser::parse_sql(&dialect, view_sql)
            .map_err(|e| IvmError::PlanParse(e.to_string()))?;

        let stmt = ast
            .into_iter()
            .next()
            .ok_or_else(|| IvmError::PlanParse("empty SQL".into()))?;

        let query = match stmt {
            sqlparser::ast::Statement::Query(q) => q,
            _ => return Err(IvmError::PlanParse("expected SELECT query".into())),
        };

        let select = match *query.body {
            SetExpr::Select(s) => s,
            _ => return Err(IvmError::PlanParse("expected SELECT body".into())),
        };

        // Extract GROUP BY columns.
        let group_by_cols: Vec<String> = match &select.group_by {
            GroupByExpr::Expressions(exprs, _) => exprs
                .iter()
                .filter_map(|expr| match expr {
                    Expr::Identifier(id) => Some(id.value.to_lowercase()),
                    Expr::CompoundIdentifier(parts) => parts.last().map(|p| p.value.to_lowercase()),
                    _ => None,
                })
                .collect(),
            GroupByExpr::All(_) => Vec::new(),
        };

        // Extract aggregates from the projection.
        let mut aggregates = Vec::new();
        for item in &select.projection {
            let (alias, expr) = match item {
                SelectItem::ExprWithAlias { expr, alias } => {
                    (Some(alias.value.clone()), expr.clone())
                }
                SelectItem::UnnamedExpr(e) => (None, e.clone()),
                _ => continue,
            };
            if let Expr::Function(f) = expr {
                let fn_name = f.name.to_string().to_uppercase();
                let kind = match fn_name.as_str() {
                    "COUNT" => AggregateKind::Count,
                    "SUM" => AggregateKind::Sum,
                    "AVG" => AggregateKind::Avg,
                    "MIN" => AggregateKind::Min,
                    "MAX" => AggregateKind::Max,
                    "STDDEV" | "STDDEV_POP" | "STDDEV_SAMP" => AggregateKind::Stddev,
                    "BOOL_AND" => AggregateKind::BoolAnd,
                    "BOOL_OR" => AggregateKind::BoolOr,
                    "BIT_AND" => AggregateKind::BitAnd,
                    "BIT_OR" => AggregateKind::BitOr,
                    "BIT_XOR" => AggregateKind::BitXor,
                    "STRING_AGG" => AggregateKind::StringAgg,
                    "ARRAY_AGG" => AggregateKind::ArrayAgg,
                    _ => continue,
                };

                let arg_list = match &f.args {
                    FunctionArguments::List(l) => l.args.as_slice(),
                    _ => &[],
                };

                let input_col = arg_list.iter().find_map(|a| match a {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::Identifier(id))) => {
                        Some(id.value.to_lowercase())
                    }
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::CompoundIdentifier(
                        parts,
                    ))) => parts.last().map(|p| p.value.to_lowercase()),
                    FunctionArg::Unnamed(FunctionArgExpr::Wildcard) => None, // COUNT(*)
                    _ => None,
                });

                let output_col = alias.unwrap_or_else(|| {
                    if let Some(ref col) = input_col {
                        format!("{}_{}", fn_name.to_lowercase(), col)
                    } else {
                        fn_name.to_lowercase()
                    }
                });

                aggregates.push(Aggregate {
                    output_col,
                    kind,
                    input_col,
                });
            }
        }

        // ── Extract JOIN clauses (v0.13) ──────────────────────────────────
        let mut join_clauses: Vec<JoinClause> = Vec::new();
        let mut input_tables: Vec<String> = Vec::new();

        // Walk the FROM clause to find table references and JOINs.
        for table_with_join in &select.from {
            // Primary table.
            if let TableFactor::Table { name, .. } = &table_with_join.relation {
                let tname = name.to_string().to_lowercase();
                if !input_tables.contains(&tname) {
                    input_tables.push(tname.clone());
                }

                // Process JOIN sub-clauses.
                let left_table = tname.clone();
                for Join {
                    relation,
                    join_operator,
                    ..
                } in &table_with_join.joins
                {
                    let right_table = match relation {
                        TableFactor::Table { name, .. } => name.to_string().to_lowercase(),
                        _ => continue,
                    };
                    if !input_tables.contains(&right_table) {
                        input_tables.push(right_table.clone());
                    }

                    // Extract equality predicate from ON clause.
                    let constraint = match join_operator {
                        SqlJoinOp::Join(c)      // bare JOIN (sqlparser 0.55+)
                        | SqlJoinOp::Inner(c)
                        | SqlJoinOp::LeftOuter(c)
                        | SqlJoinOp::RightOuter(c)
                        | SqlJoinOp::FullOuter(c)
                        | SqlJoinOp::LeftSemi(c)
                        | SqlJoinOp::RightSemi(c)
                        | SqlJoinOp::LeftAnti(c)
                        | SqlJoinOp::RightAnti(c) => c,
                        _ => continue,
                    };

                    let on_expr = match constraint {
                        JoinConstraint::On(e) => e,
                        _ => continue,
                    };

                    // Only equality predicates are supported.
                    let (left_col, right_col) =
                        match extract_eq_cols(on_expr, &left_table, &right_table) {
                            Some(pair) => pair,
                            None => continue,
                        };

                    join_clauses.push(JoinClause {
                        left_table: left_table.clone(),
                        right_table,
                        left_col,
                        right_col,
                        strategy: JoinStrategy::Broadcast, // default; overridden at runtime
                        broadcast_threshold: DEFAULT_BROADCAST_THRESHOLD,
                    });
                }
            }
        }

        Ok(IvmPlan {
            view_sql: view_sql.to_string(),
            group_by_cols,
            aggregates,
            joins: join_clauses,
            input_tables,
            broadcast_threshold: DEFAULT_BROADCAST_THRESHOLD,
        })
    }

    /// Compile the IVM plan with volatility validation.
    ///
    /// Walks the view SQL expression tree and checks each function call
    /// against the static volatility lookup table.
    ///
    /// - VOLATILE functions: return `SQLSTATE 0A000` at view creation
    /// - STABLE functions: emit warning, accept
    /// - IMMUTABLE: always accepted
    /// - Unknown: treated as VOLATILE unless `allow_unknown_functions` is set
    pub fn compile(view_sql: &str, allow_unknown_functions: bool) -> Result<Self, IvmError> {
        // First parse.
        let plan = Self::parse(view_sql)?;

        // Walk the AST to find function calls and validate volatility.
        use sqlparser::ast::{SetExpr, Statement};
        use sqlparser::dialect::GenericDialect;
        use sqlparser::parser::Parser;

        let dialect = GenericDialect {};
        let ast = Parser::parse_sql(&dialect, view_sql)
            .map_err(|e| IvmError::PlanParse(e.to_string()))?;

        for stmt in &ast {
            if let Statement::Query(q) = stmt {
                if let SetExpr::Select(select) = q.body.as_ref() {
                    // Check projection expressions.
                    for item in &select.projection {
                        let expr = match item {
                            sqlparser::ast::SelectItem::ExprWithAlias { expr, .. } => Some(expr),
                            sqlparser::ast::SelectItem::UnnamedExpr(e) => Some(e),
                            _ => None,
                        };
                        if let Some(e) = expr {
                            check_expr_volatility(e, allow_unknown_functions)?;
                        }
                    }
                    // Check WHERE clause.
                    if let Some(ref where_clause) = select.selection {
                        check_expr_volatility(where_clause, allow_unknown_functions)?;
                    }
                    // Check HAVING clause.
                    if let Some(ref having) = select.having {
                        check_expr_volatility(having, allow_unknown_functions)?;
                    }
                }
            }
        }

        Ok(plan)
    }
}

/// Recursively check expression tree for volatile function calls.
fn check_expr_volatility(expr: &sqlparser::ast::Expr, allow_unknown: bool) -> Result<(), IvmError> {
    use sqlparser::ast::Expr;

    match expr {
        Expr::Function(f) => {
            let fn_name = f.name.to_string().to_lowercase();
            // Skip known aggregate functions (handled separately).
            let aggregates = [
                "count", "sum", "avg", "min", "max", "stddev", "stddev_pop",
                "stddev_samp", "bool_and", "bool_or", "bit_and", "bit_or",
                "bit_xor", "string_agg", "array_agg",
            ];
            if !aggregates.contains(&fn_name.as_str()) {
                let vol = volatility::volatility_of(&fn_name);
                match vol {
                    Volatility::Volatile => {
                        return Err(IvmError::VolatileFunction(fn_name));
                    }
                    Volatility::Stable => {
                        tracing::warn!(
                            "STABLE function `{fn_name}` used in materialized view; \
                             consider capture-semantics path (v0.16)"
                        );
                    }
                    Volatility::Unknown if !allow_unknown => {
                        return Err(IvmError::UnknownVolatility(fn_name));
                    }
                    Volatility::Immutable | Volatility::Unknown => {}
                }
            }
            // Check function arguments recursively.
            if let sqlparser::ast::FunctionArguments::List(args) = &f.args {
                for arg in &args.args {
                    if let sqlparser::ast::FunctionArg::Unnamed(
                        sqlparser::ast::FunctionArgExpr::Expr(e),
                    ) = arg
                    {
                        check_expr_volatility(e, allow_unknown)?;
                    }
                }
            }
            Ok(())
        }
        Expr::BinaryOp { left, right, .. } => {
            check_expr_volatility(left, allow_unknown)?;
            check_expr_volatility(right, allow_unknown)?;
            Ok(())
        }
        Expr::UnaryOp { expr: inner, .. } => check_expr_volatility(inner, allow_unknown),
        Expr::Nested(inner) => check_expr_volatility(inner, allow_unknown),
        Expr::IsNull(inner) | Expr::IsNotNull(inner) => {
            check_expr_volatility(inner, allow_unknown)
        }
        Expr::Between {
            expr: e,
            low,
            high,
            ..
        } => {
            check_expr_volatility(e, allow_unknown)?;
            check_expr_volatility(low, allow_unknown)?;
            check_expr_volatility(high, allow_unknown)?;
            Ok(())
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(op) = operand {
                check_expr_volatility(op, allow_unknown)?;
            }
            for case_when in conditions {
                check_expr_volatility(&case_when.condition, allow_unknown)?;
                check_expr_volatility(&case_when.result, allow_unknown)?;
            }
            if let Some(el) = else_result {
                check_expr_volatility(el, allow_unknown)?;
            }
            Ok(())
        }
        Expr::Cast { expr: inner, .. } => check_expr_volatility(inner, allow_unknown),
        _ => Ok(()),
    }
}

/// Extract left/right column names from an equality predicate of the form
/// `left_table.col = right_table.col` (or bare `col = col`).
///
/// Returns `None` if the predicate is not a simple equality.
fn extract_eq_cols(
    expr: &sqlparser::ast::Expr,
    left_table: &str,
    right_table: &str,
) -> Option<(String, String)> {
    use sqlparser::ast::{BinaryOperator, Expr};

    if let Expr::BinaryOp { left, op, right } = expr {
        if *op != BinaryOperator::Eq {
            return None;
        }
        let lcol = col_name(left, left_table).or_else(|| col_name(left, right_table))?;
        let rcol = col_name(right, right_table).or_else(|| col_name(right, left_table))?;
        return Some((lcol, rcol));
    }
    None
}

/// Extract a bare column name from an Identifier or CompoundIdentifier that
/// optionally qualifies a table name.
fn col_name(expr: &sqlparser::ast::Expr, _table_hint: &str) -> Option<String> {
    use sqlparser::ast::Expr;
    match expr {
        Expr::Identifier(id) => Some(id.value.to_lowercase()),
        Expr::CompoundIdentifier(parts) => parts.last().map(|p| p.value.to_lowercase()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_count_star_group_by() {
        let sql = "SELECT region, COUNT(*) AS cnt FROM sales GROUP BY region";
        let plan = IvmPlan::parse(sql).unwrap();
        assert_eq!(plan.group_by_cols, vec!["region"]);
        assert_eq!(plan.aggregates.len(), 1);
        assert_eq!(plan.aggregates[0].kind, AggregateKind::Count);
        assert_eq!(plan.aggregates[0].output_col, "cnt");
        assert!(plan.joins.is_empty(), "single-table plan has no JOINs");
    }

    #[test]
    fn parse_sum_aggregate() {
        let sql = "SELECT dept, SUM(amount) AS total FROM orders GROUP BY dept";
        let plan = IvmPlan::parse(sql).unwrap();
        assert_eq!(plan.group_by_cols, vec!["dept"]);
        assert_eq!(plan.aggregates[0].kind, AggregateKind::Sum);
        assert_eq!(plan.aggregates[0].input_col, Some("amount".to_string()));
    }

    #[test]
    fn parse_multi_agg() {
        let sql = "SELECT dept, MIN(salary) AS lo, MAX(salary) AS hi FROM emp GROUP BY dept";
        let plan = IvmPlan::parse(sql).unwrap();
        assert_eq!(plan.aggregates.len(), 2);
        assert_eq!(plan.aggregates[0].kind, AggregateKind::Min);
        assert_eq!(plan.aggregates[1].kind, AggregateKind::Max);
    }

    #[test]
    fn invalid_sql_returns_error() {
        assert!(IvmPlan::parse("NOT SQL").is_err());
    }

    #[test]
    fn parse_join_extracts_clause() {
        let sql = "SELECT e.cat_id, COUNT(*) AS cnt \
                   FROM events e \
                   JOIN categories c ON e.cat_id = c.cat_id \
                   GROUP BY e.cat_id";
        let plan = IvmPlan::parse(sql).unwrap();
        assert_eq!(plan.joins.len(), 1);
        let j = &plan.joins[0];
        assert_eq!(j.left_table, "events");
        assert_eq!(j.right_table, "categories");
        assert_eq!(j.left_col, "cat_id");
        assert_eq!(j.right_col, "cat_id");
        assert_eq!(j.strategy, JoinStrategy::Broadcast); // default
    }

    #[test]
    fn parse_join_populates_input_tables() {
        let sql = "SELECT o.order_id, COUNT(*) AS line_count \
                   FROM orders o \
                   JOIN lineitem l ON o.order_id = l.order_id \
                   GROUP BY o.order_id";
        let plan = IvmPlan::parse(sql).unwrap();
        assert!(plan.input_tables.contains(&"orders".to_string()));
        assert!(plan.input_tables.contains(&"lineitem".to_string()));
    }
}
