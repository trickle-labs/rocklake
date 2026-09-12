_rocklake() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="rocklake"
                ;;
            rocklake,backup)
                cmd="rocklake__subcmd__backup"
                ;;
            rocklake,capacity)
                cmd="rocklake__subcmd__capacity"
                ;;
            rocklake,catalog)
                cmd="rocklake__subcmd__catalog"
                ;;
            rocklake,catalogs)
                cmd="rocklake__subcmd__catalogs"
                ;;
            rocklake,checkpoint)
                cmd="rocklake__subcmd__checkpoint"
                ;;
            rocklake,completions)
                cmd="rocklake__subcmd__completions"
                ;;
            rocklake,config)
                cmd="rocklake__subcmd__config"
                ;;
            rocklake,corpus)
                cmd="rocklake__subcmd__corpus"
                ;;
            rocklake,debug)
                cmd="rocklake__subcmd__debug"
                ;;
            rocklake,diagnose)
                cmd="rocklake__subcmd__diagnose"
                ;;
            rocklake,doctor)
                cmd="rocklake__subcmd__doctor"
                ;;
            rocklake,excise)
                cmd="rocklake__subcmd__excise"
                ;;
            rocklake,export)
                cmd="rocklake__subcmd__export"
                ;;
            rocklake,export-catalog)
                cmd="rocklake__subcmd__export__subcmd__catalog"
                ;;
            rocklake,gc)
                cmd="rocklake__subcmd__gc"
                ;;
            rocklake,help)
                cmd="rocklake__subcmd__help"
                ;;
            rocklake,import)
                cmd="rocklake__subcmd__import"
                ;;
            rocklake,inspect)
                cmd="rocklake__subcmd__inspect"
                ;;
            rocklake,migrate)
                cmd="rocklake__subcmd__migrate"
                ;;
            rocklake,migrate-from-ducklake)
                cmd="rocklake__subcmd__migrate__subcmd__from__subcmd__ducklake"
                ;;
            rocklake,pg-migrate)
                cmd="rocklake__subcmd__pg__subcmd__migrate"
                ;;
            rocklake,rebuild)
                cmd="rocklake__subcmd__rebuild"
                ;;
            rocklake,registry)
                cmd="rocklake__subcmd__registry"
                ;;
            rocklake,repair)
                cmd="rocklake__subcmd__repair"
                ;;
            rocklake,restore)
                cmd="rocklake__subcmd__restore"
                ;;
            rocklake,serve)
                cmd="rocklake__subcmd__serve"
                ;;
            rocklake,status)
                cmd="rocklake__subcmd__status"
                ;;
            rocklake,support)
                cmd="rocklake__subcmd__support"
                ;;
            rocklake,sweep-orphans)
                cmd="rocklake__subcmd__sweep__subcmd__orphans"
                ;;
            rocklake,tune)
                cmd="rocklake__subcmd__tune"
                ;;
            rocklake,verify)
                cmd="rocklake__subcmd__verify"
                ;;
            rocklake,warmup)
                cmd="rocklake__subcmd__warmup"
                ;;
            rocklake__subcmd__backup,create)
                cmd="rocklake__subcmd__backup__subcmd__create"
                ;;
            rocklake__subcmd__backup,help)
                cmd="rocklake__subcmd__backup__subcmd__help"
                ;;
            rocklake__subcmd__backup,inspect)
                cmd="rocklake__subcmd__backup__subcmd__inspect"
                ;;
            rocklake__subcmd__backup__subcmd__help,create)
                cmd="rocklake__subcmd__backup__subcmd__help__subcmd__create"
                ;;
            rocklake__subcmd__backup__subcmd__help,help)
                cmd="rocklake__subcmd__backup__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__backup__subcmd__help,inspect)
                cmd="rocklake__subcmd__backup__subcmd__help__subcmd__inspect"
                ;;
            rocklake__subcmd__capacity,help)
                cmd="rocklake__subcmd__capacity__subcmd__help"
                ;;
            rocklake__subcmd__capacity,report)
                cmd="rocklake__subcmd__capacity__subcmd__report"
                ;;
            rocklake__subcmd__capacity__subcmd__help,help)
                cmd="rocklake__subcmd__capacity__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__capacity__subcmd__help,report)
                cmd="rocklake__subcmd__capacity__subcmd__help__subcmd__report"
                ;;
            rocklake__subcmd__catalog,backup)
                cmd="rocklake__subcmd__catalog__subcmd__backup"
                ;;
            rocklake__subcmd__catalog,backup-set)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set"
                ;;
            rocklake__subcmd__catalog,checkpoint)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint"
                ;;
            rocklake__subcmd__catalog,excise)
                cmd="rocklake__subcmd__catalog__subcmd__excise"
                ;;
            rocklake__subcmd__catalog,export)
                cmd="rocklake__subcmd__catalog__subcmd__export"
                ;;
            rocklake__subcmd__catalog,export-catalog)
                cmd="rocklake__subcmd__catalog__subcmd__export__subcmd__catalog"
                ;;
            rocklake__subcmd__catalog,gc)
                cmd="rocklake__subcmd__catalog__subcmd__gc"
                ;;
            rocklake__subcmd__catalog,help)
                cmd="rocklake__subcmd__catalog__subcmd__help"
                ;;
            rocklake__subcmd__catalog,import)
                cmd="rocklake__subcmd__catalog__subcmd__import"
                ;;
            rocklake__subcmd__catalog,jobs)
                cmd="rocklake__subcmd__catalog__subcmd__jobs"
                ;;
            rocklake__subcmd__catalog,maintenance)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance"
                ;;
            rocklake__subcmd__catalog,migrate)
                cmd="rocklake__subcmd__catalog__subcmd__migrate"
                ;;
            rocklake__subcmd__catalog,recovery)
                cmd="rocklake__subcmd__catalog__subcmd__recovery"
                ;;
            rocklake__subcmd__catalog,repair)
                cmd="rocklake__subcmd__catalog__subcmd__repair"
                ;;
            rocklake__subcmd__catalog,restore)
                cmd="rocklake__subcmd__catalog__subcmd__restore"
                ;;
            rocklake__subcmd__catalog,verify)
                cmd="rocklake__subcmd__catalog__subcmd__verify"
                ;;
            rocklake__subcmd__catalog__subcmd__backup,create)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__create"
                ;;
            rocklake__subcmd__catalog__subcmd__backup,help)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__backup,inspect)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__inspect"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__help,create)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__help__subcmd__create"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__help,inspect)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__help__subcmd__inspect"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set,apply)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set,create)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__create"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set,help)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set,inspect)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__inspect"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set,plan)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help,apply)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help,create)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__create"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help,inspect)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__inspect"
                ;;
            rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help,plan)
                cmd="rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint,create)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__create"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint,help)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint,list)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__list"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint,pin)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__pin"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint,pins)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__pins"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint,restore)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__restore"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint,unpin)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__unpin"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help,create)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__create"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help,list)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__list"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help,pin)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__pin"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help,pins)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__pins"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help,restore)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__restore"
                ;;
            rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help,unpin)
                cmd="rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__unpin"
                ;;
            rocklake__subcmd__catalog__subcmd__excise,apply)
                cmd="rocklake__subcmd__catalog__subcmd__excise__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__excise,help)
                cmd="rocklake__subcmd__catalog__subcmd__excise__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__excise,plan)
                cmd="rocklake__subcmd__catalog__subcmd__excise__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__excise__subcmd__help,apply)
                cmd="rocklake__subcmd__catalog__subcmd__excise__subcmd__help__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__excise__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__excise__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__excise__subcmd__help,plan)
                cmd="rocklake__subcmd__catalog__subcmd__excise__subcmd__help__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__gc,apply)
                cmd="rocklake__subcmd__catalog__subcmd__gc__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__gc,help)
                cmd="rocklake__subcmd__catalog__subcmd__gc__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__gc,plan)
                cmd="rocklake__subcmd__catalog__subcmd__gc__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__gc__subcmd__help,apply)
                cmd="rocklake__subcmd__catalog__subcmd__gc__subcmd__help__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__gc__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__gc__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__gc__subcmd__help,plan)
                cmd="rocklake__subcmd__catalog__subcmd__gc__subcmd__help__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__help,backup)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__backup"
                ;;
            rocklake__subcmd__catalog__subcmd__help,backup-set)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set"
                ;;
            rocklake__subcmd__catalog__subcmd__help,checkpoint)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint"
                ;;
            rocklake__subcmd__catalog__subcmd__help,excise)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__excise"
                ;;
            rocklake__subcmd__catalog__subcmd__help,export)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__export"
                ;;
            rocklake__subcmd__catalog__subcmd__help,export-catalog)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__export__subcmd__catalog"
                ;;
            rocklake__subcmd__catalog__subcmd__help,gc)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__gc"
                ;;
            rocklake__subcmd__catalog__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__help,import)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__import"
                ;;
            rocklake__subcmd__catalog__subcmd__help,jobs)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__jobs"
                ;;
            rocklake__subcmd__catalog__subcmd__help,maintenance)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance"
                ;;
            rocklake__subcmd__catalog__subcmd__help,migrate)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__migrate"
                ;;
            rocklake__subcmd__catalog__subcmd__help,recovery)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__recovery"
                ;;
            rocklake__subcmd__catalog__subcmd__help,repair)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__repair"
                ;;
            rocklake__subcmd__catalog__subcmd__help,restore)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__restore"
                ;;
            rocklake__subcmd__catalog__subcmd__help,verify)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__verify"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__backup,create)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__create"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__backup,inspect)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__inspect"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set,apply)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set,create)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set__subcmd__create"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set,inspect)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set__subcmd__inspect"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set,plan)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint,create)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__create"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint,list)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__list"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint,pin)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__pin"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint,pins)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__pins"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint,restore)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__restore"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint,unpin)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__unpin"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__excise,apply)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__excise__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__excise,plan)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__excise__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__gc,apply)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__gc__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__gc,plan)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__gc__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__jobs,cancel)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__jobs__subcmd__cancel"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__jobs,list)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__jobs__subcmd__list"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__jobs,resume)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__jobs__subcmd__resume"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__jobs,status)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__jobs__subcmd__status"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance,list)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance__subcmd__list"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance,remove)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance__subcmd__remove"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance,run)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance__subcmd__run"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance,schedule)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance__subcmd__schedule"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__recovery,report)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__recovery__subcmd__report"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__restore,apply)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__restore__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__restore,plan)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__restore__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__verify,catalog)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__verify__subcmd__catalog"
                ;;
            rocklake__subcmd__catalog__subcmd__help__subcmd__verify,data-files)
                cmd="rocklake__subcmd__catalog__subcmd__help__subcmd__verify__subcmd__data__subcmd__files"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs,cancel)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__cancel"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs,help)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs,list)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__list"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs,resume)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__resume"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs,status)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__status"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs__subcmd__help,cancel)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__cancel"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs__subcmd__help,list)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__list"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs__subcmd__help,resume)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__resume"
                ;;
            rocklake__subcmd__catalog__subcmd__jobs__subcmd__help,status)
                cmd="rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__status"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance,help)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance,list)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__list"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance,remove)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__remove"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance,run)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__run"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance,schedule)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__schedule"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help,list)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__list"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help,remove)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__remove"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help,run)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__run"
                ;;
            rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help,schedule)
                cmd="rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__schedule"
                ;;
            rocklake__subcmd__catalog__subcmd__recovery,help)
                cmd="rocklake__subcmd__catalog__subcmd__recovery__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__recovery,report)
                cmd="rocklake__subcmd__catalog__subcmd__recovery__subcmd__report"
                ;;
            rocklake__subcmd__catalog__subcmd__recovery__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__recovery__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__recovery__subcmd__help,report)
                cmd="rocklake__subcmd__catalog__subcmd__recovery__subcmd__help__subcmd__report"
                ;;
            rocklake__subcmd__catalog__subcmd__restore,apply)
                cmd="rocklake__subcmd__catalog__subcmd__restore__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__restore,help)
                cmd="rocklake__subcmd__catalog__subcmd__restore__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__restore,plan)
                cmd="rocklake__subcmd__catalog__subcmd__restore__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__restore__subcmd__help,apply)
                cmd="rocklake__subcmd__catalog__subcmd__restore__subcmd__help__subcmd__apply"
                ;;
            rocklake__subcmd__catalog__subcmd__restore__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__restore__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__restore__subcmd__help,plan)
                cmd="rocklake__subcmd__catalog__subcmd__restore__subcmd__help__subcmd__plan"
                ;;
            rocklake__subcmd__catalog__subcmd__verify,catalog)
                cmd="rocklake__subcmd__catalog__subcmd__verify__subcmd__catalog"
                ;;
            rocklake__subcmd__catalog__subcmd__verify,data-files)
                cmd="rocklake__subcmd__catalog__subcmd__verify__subcmd__data__subcmd__files"
                ;;
            rocklake__subcmd__catalog__subcmd__verify,help)
                cmd="rocklake__subcmd__catalog__subcmd__verify__subcmd__help"
                ;;
            rocklake__subcmd__catalog__subcmd__verify__subcmd__help,catalog)
                cmd="rocklake__subcmd__catalog__subcmd__verify__subcmd__help__subcmd__catalog"
                ;;
            rocklake__subcmd__catalog__subcmd__verify__subcmd__help,data-files)
                cmd="rocklake__subcmd__catalog__subcmd__verify__subcmd__help__subcmd__data__subcmd__files"
                ;;
            rocklake__subcmd__catalog__subcmd__verify__subcmd__help,help)
                cmd="rocklake__subcmd__catalog__subcmd__verify__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalogs,activate)
                cmd="rocklake__subcmd__catalogs__subcmd__activate"
                ;;
            rocklake__subcmd__catalogs,create)
                cmd="rocklake__subcmd__catalogs__subcmd__create"
                ;;
            rocklake__subcmd__catalogs,disable)
                cmd="rocklake__subcmd__catalogs__subcmd__disable"
                ;;
            rocklake__subcmd__catalogs,enable)
                cmd="rocklake__subcmd__catalogs__subcmd__enable"
                ;;
            rocklake__subcmd__catalogs,help)
                cmd="rocklake__subcmd__catalogs__subcmd__help"
                ;;
            rocklake__subcmd__catalogs,list)
                cmd="rocklake__subcmd__catalogs__subcmd__list"
                ;;
            rocklake__subcmd__catalogs,promote)
                cmd="rocklake__subcmd__catalogs__subcmd__promote"
                ;;
            rocklake__subcmd__catalogs,register)
                cmd="rocklake__subcmd__catalogs__subcmd__register"
                ;;
            rocklake__subcmd__catalogs,remove)
                cmd="rocklake__subcmd__catalogs__subcmd__remove"
                ;;
            rocklake__subcmd__catalogs,rename)
                cmd="rocklake__subcmd__catalogs__subcmd__rename"
                ;;
            rocklake__subcmd__catalogs,set-mode)
                cmd="rocklake__subcmd__catalogs__subcmd__set__subcmd__mode"
                ;;
            rocklake__subcmd__catalogs,status)
                cmd="rocklake__subcmd__catalogs__subcmd__status"
                ;;
            rocklake__subcmd__catalogs,validate)
                cmd="rocklake__subcmd__catalogs__subcmd__validate"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,activate)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__activate"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,create)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__create"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,disable)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__disable"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,enable)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__enable"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,help)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,list)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__list"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,promote)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__promote"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,register)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__register"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,remove)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__remove"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,rename)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__rename"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,set-mode)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__set__subcmd__mode"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,status)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__status"
                ;;
            rocklake__subcmd__catalogs__subcmd__help,validate)
                cmd="rocklake__subcmd__catalogs__subcmd__help__subcmd__validate"
                ;;
            rocklake__subcmd__checkpoint,create)
                cmd="rocklake__subcmd__checkpoint__subcmd__create"
                ;;
            rocklake__subcmd__checkpoint,help)
                cmd="rocklake__subcmd__checkpoint__subcmd__help"
                ;;
            rocklake__subcmd__checkpoint,list)
                cmd="rocklake__subcmd__checkpoint__subcmd__list"
                ;;
            rocklake__subcmd__checkpoint,pin)
                cmd="rocklake__subcmd__checkpoint__subcmd__pin"
                ;;
            rocklake__subcmd__checkpoint,pins)
                cmd="rocklake__subcmd__checkpoint__subcmd__pins"
                ;;
            rocklake__subcmd__checkpoint,restore)
                cmd="rocklake__subcmd__checkpoint__subcmd__restore"
                ;;
            rocklake__subcmd__checkpoint,unpin)
                cmd="rocklake__subcmd__checkpoint__subcmd__unpin"
                ;;
            rocklake__subcmd__checkpoint__subcmd__help,create)
                cmd="rocklake__subcmd__checkpoint__subcmd__help__subcmd__create"
                ;;
            rocklake__subcmd__checkpoint__subcmd__help,help)
                cmd="rocklake__subcmd__checkpoint__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__checkpoint__subcmd__help,list)
                cmd="rocklake__subcmd__checkpoint__subcmd__help__subcmd__list"
                ;;
            rocklake__subcmd__checkpoint__subcmd__help,pin)
                cmd="rocklake__subcmd__checkpoint__subcmd__help__subcmd__pin"
                ;;
            rocklake__subcmd__checkpoint__subcmd__help,pins)
                cmd="rocklake__subcmd__checkpoint__subcmd__help__subcmd__pins"
                ;;
            rocklake__subcmd__checkpoint__subcmd__help,restore)
                cmd="rocklake__subcmd__checkpoint__subcmd__help__subcmd__restore"
                ;;
            rocklake__subcmd__checkpoint__subcmd__help,unpin)
                cmd="rocklake__subcmd__checkpoint__subcmd__help__subcmd__unpin"
                ;;
            rocklake__subcmd__config,check)
                cmd="rocklake__subcmd__config__subcmd__check"
                ;;
            rocklake__subcmd__config,example)
                cmd="rocklake__subcmd__config__subcmd__example"
                ;;
            rocklake__subcmd__config,help)
                cmd="rocklake__subcmd__config__subcmd__help"
                ;;
            rocklake__subcmd__config__subcmd__help,check)
                cmd="rocklake__subcmd__config__subcmd__help__subcmd__check"
                ;;
            rocklake__subcmd__config__subcmd__help,example)
                cmd="rocklake__subcmd__config__subcmd__help__subcmd__example"
                ;;
            rocklake__subcmd__config__subcmd__help,help)
                cmd="rocklake__subcmd__config__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__corpus,diff)
                cmd="rocklake__subcmd__corpus__subcmd__diff"
                ;;
            rocklake__subcmd__corpus,help)
                cmd="rocklake__subcmd__corpus__subcmd__help"
                ;;
            rocklake__subcmd__corpus,validate)
                cmd="rocklake__subcmd__corpus__subcmd__validate"
                ;;
            rocklake__subcmd__corpus__subcmd__help,diff)
                cmd="rocklake__subcmd__corpus__subcmd__help__subcmd__diff"
                ;;
            rocklake__subcmd__corpus__subcmd__help,help)
                cmd="rocklake__subcmd__corpus__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__corpus__subcmd__help,validate)
                cmd="rocklake__subcmd__corpus__subcmd__help__subcmd__validate"
                ;;
            rocklake__subcmd__debug,corpus)
                cmd="rocklake__subcmd__debug__subcmd__corpus"
                ;;
            rocklake__subcmd__debug,diagnose)
                cmd="rocklake__subcmd__debug__subcmd__diagnose"
                ;;
            rocklake__subcmd__debug,help)
                cmd="rocklake__subcmd__debug__subcmd__help"
                ;;
            rocklake__subcmd__debug,inspect)
                cmd="rocklake__subcmd__debug__subcmd__inspect"
                ;;
            rocklake__subcmd__debug,migrate-from-ducklake)
                cmd="rocklake__subcmd__debug__subcmd__migrate__subcmd__from__subcmd__ducklake"
                ;;
            rocklake__subcmd__debug,pg-migrate)
                cmd="rocklake__subcmd__debug__subcmd__pg__subcmd__migrate"
                ;;
            rocklake__subcmd__debug,rebuild)
                cmd="rocklake__subcmd__debug__subcmd__rebuild"
                ;;
            rocklake__subcmd__debug,sweep-orphans)
                cmd="rocklake__subcmd__debug__subcmd__sweep__subcmd__orphans"
                ;;
            rocklake__subcmd__debug,tune)
                cmd="rocklake__subcmd__debug__subcmd__tune"
                ;;
            rocklake__subcmd__debug,warmup)
                cmd="rocklake__subcmd__debug__subcmd__warmup"
                ;;
            rocklake__subcmd__debug__subcmd__corpus,diff)
                cmd="rocklake__subcmd__debug__subcmd__corpus__subcmd__diff"
                ;;
            rocklake__subcmd__debug__subcmd__corpus,help)
                cmd="rocklake__subcmd__debug__subcmd__corpus__subcmd__help"
                ;;
            rocklake__subcmd__debug__subcmd__corpus,validate)
                cmd="rocklake__subcmd__debug__subcmd__corpus__subcmd__validate"
                ;;
            rocklake__subcmd__debug__subcmd__corpus__subcmd__help,diff)
                cmd="rocklake__subcmd__debug__subcmd__corpus__subcmd__help__subcmd__diff"
                ;;
            rocklake__subcmd__debug__subcmd__corpus__subcmd__help,help)
                cmd="rocklake__subcmd__debug__subcmd__corpus__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__debug__subcmd__corpus__subcmd__help,validate)
                cmd="rocklake__subcmd__debug__subcmd__corpus__subcmd__help__subcmd__validate"
                ;;
            rocklake__subcmd__debug__subcmd__help,corpus)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__corpus"
                ;;
            rocklake__subcmd__debug__subcmd__help,diagnose)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__diagnose"
                ;;
            rocklake__subcmd__debug__subcmd__help,help)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__debug__subcmd__help,inspect)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__inspect"
                ;;
            rocklake__subcmd__debug__subcmd__help,migrate-from-ducklake)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__migrate__subcmd__from__subcmd__ducklake"
                ;;
            rocklake__subcmd__debug__subcmd__help,pg-migrate)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__pg__subcmd__migrate"
                ;;
            rocklake__subcmd__debug__subcmd__help,rebuild)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__rebuild"
                ;;
            rocklake__subcmd__debug__subcmd__help,sweep-orphans)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__sweep__subcmd__orphans"
                ;;
            rocklake__subcmd__debug__subcmd__help,tune)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__tune"
                ;;
            rocklake__subcmd__debug__subcmd__help,warmup)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__warmup"
                ;;
            rocklake__subcmd__debug__subcmd__help__subcmd__corpus,diff)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__corpus__subcmd__diff"
                ;;
            rocklake__subcmd__debug__subcmd__help__subcmd__corpus,validate)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__corpus__subcmd__validate"
                ;;
            rocklake__subcmd__debug__subcmd__help__subcmd__inspect,api-costs)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__inspect__subcmd__api__subcmd__costs"
                ;;
            rocklake__subcmd__debug__subcmd__help__subcmd__inspect,cache-utilization)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__inspect__subcmd__cache__subcmd__utilization"
                ;;
            rocklake__subcmd__debug__subcmd__help__subcmd__inspect,snapshot)
                cmd="rocklake__subcmd__debug__subcmd__help__subcmd__inspect__subcmd__snapshot"
                ;;
            rocklake__subcmd__debug__subcmd__inspect,api-costs)
                cmd="rocklake__subcmd__debug__subcmd__inspect__subcmd__api__subcmd__costs"
                ;;
            rocklake__subcmd__debug__subcmd__inspect,cache-utilization)
                cmd="rocklake__subcmd__debug__subcmd__inspect__subcmd__cache__subcmd__utilization"
                ;;
            rocklake__subcmd__debug__subcmd__inspect,help)
                cmd="rocklake__subcmd__debug__subcmd__inspect__subcmd__help"
                ;;
            rocklake__subcmd__debug__subcmd__inspect,snapshot)
                cmd="rocklake__subcmd__debug__subcmd__inspect__subcmd__snapshot"
                ;;
            rocklake__subcmd__debug__subcmd__inspect__subcmd__help,api-costs)
                cmd="rocklake__subcmd__debug__subcmd__inspect__subcmd__help__subcmd__api__subcmd__costs"
                ;;
            rocklake__subcmd__debug__subcmd__inspect__subcmd__help,cache-utilization)
                cmd="rocklake__subcmd__debug__subcmd__inspect__subcmd__help__subcmd__cache__subcmd__utilization"
                ;;
            rocklake__subcmd__debug__subcmd__inspect__subcmd__help,help)
                cmd="rocklake__subcmd__debug__subcmd__inspect__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__debug__subcmd__inspect__subcmd__help,snapshot)
                cmd="rocklake__subcmd__debug__subcmd__inspect__subcmd__help__subcmd__snapshot"
                ;;
            rocklake__subcmd__excise,apply)
                cmd="rocklake__subcmd__excise__subcmd__apply"
                ;;
            rocklake__subcmd__excise,help)
                cmd="rocklake__subcmd__excise__subcmd__help"
                ;;
            rocklake__subcmd__excise,plan)
                cmd="rocklake__subcmd__excise__subcmd__plan"
                ;;
            rocklake__subcmd__excise__subcmd__help,apply)
                cmd="rocklake__subcmd__excise__subcmd__help__subcmd__apply"
                ;;
            rocklake__subcmd__excise__subcmd__help,help)
                cmd="rocklake__subcmd__excise__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__excise__subcmd__help,plan)
                cmd="rocklake__subcmd__excise__subcmd__help__subcmd__plan"
                ;;
            rocklake__subcmd__gc,apply)
                cmd="rocklake__subcmd__gc__subcmd__apply"
                ;;
            rocklake__subcmd__gc,help)
                cmd="rocklake__subcmd__gc__subcmd__help"
                ;;
            rocklake__subcmd__gc,plan)
                cmd="rocklake__subcmd__gc__subcmd__plan"
                ;;
            rocklake__subcmd__gc__subcmd__help,apply)
                cmd="rocklake__subcmd__gc__subcmd__help__subcmd__apply"
                ;;
            rocklake__subcmd__gc__subcmd__help,help)
                cmd="rocklake__subcmd__gc__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__gc__subcmd__help,plan)
                cmd="rocklake__subcmd__gc__subcmd__help__subcmd__plan"
                ;;
            rocklake__subcmd__help,backup)
                cmd="rocklake__subcmd__help__subcmd__backup"
                ;;
            rocklake__subcmd__help,capacity)
                cmd="rocklake__subcmd__help__subcmd__capacity"
                ;;
            rocklake__subcmd__help,catalog)
                cmd="rocklake__subcmd__help__subcmd__catalog"
                ;;
            rocklake__subcmd__help,catalogs)
                cmd="rocklake__subcmd__help__subcmd__catalogs"
                ;;
            rocklake__subcmd__help,checkpoint)
                cmd="rocklake__subcmd__help__subcmd__checkpoint"
                ;;
            rocklake__subcmd__help,completions)
                cmd="rocklake__subcmd__help__subcmd__completions"
                ;;
            rocklake__subcmd__help,config)
                cmd="rocklake__subcmd__help__subcmd__config"
                ;;
            rocklake__subcmd__help,corpus)
                cmd="rocklake__subcmd__help__subcmd__corpus"
                ;;
            rocklake__subcmd__help,debug)
                cmd="rocklake__subcmd__help__subcmd__debug"
                ;;
            rocklake__subcmd__help,diagnose)
                cmd="rocklake__subcmd__help__subcmd__diagnose"
                ;;
            rocklake__subcmd__help,doctor)
                cmd="rocklake__subcmd__help__subcmd__doctor"
                ;;
            rocklake__subcmd__help,excise)
                cmd="rocklake__subcmd__help__subcmd__excise"
                ;;
            rocklake__subcmd__help,export)
                cmd="rocklake__subcmd__help__subcmd__export"
                ;;
            rocklake__subcmd__help,export-catalog)
                cmd="rocklake__subcmd__help__subcmd__export__subcmd__catalog"
                ;;
            rocklake__subcmd__help,gc)
                cmd="rocklake__subcmd__help__subcmd__gc"
                ;;
            rocklake__subcmd__help,help)
                cmd="rocklake__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__help,import)
                cmd="rocklake__subcmd__help__subcmd__import"
                ;;
            rocklake__subcmd__help,inspect)
                cmd="rocklake__subcmd__help__subcmd__inspect"
                ;;
            rocklake__subcmd__help,migrate)
                cmd="rocklake__subcmd__help__subcmd__migrate"
                ;;
            rocklake__subcmd__help,migrate-from-ducklake)
                cmd="rocklake__subcmd__help__subcmd__migrate__subcmd__from__subcmd__ducklake"
                ;;
            rocklake__subcmd__help,pg-migrate)
                cmd="rocklake__subcmd__help__subcmd__pg__subcmd__migrate"
                ;;
            rocklake__subcmd__help,rebuild)
                cmd="rocklake__subcmd__help__subcmd__rebuild"
                ;;
            rocklake__subcmd__help,registry)
                cmd="rocklake__subcmd__help__subcmd__registry"
                ;;
            rocklake__subcmd__help,repair)
                cmd="rocklake__subcmd__help__subcmd__repair"
                ;;
            rocklake__subcmd__help,restore)
                cmd="rocklake__subcmd__help__subcmd__restore"
                ;;
            rocklake__subcmd__help,serve)
                cmd="rocklake__subcmd__help__subcmd__serve"
                ;;
            rocklake__subcmd__help,status)
                cmd="rocklake__subcmd__help__subcmd__status"
                ;;
            rocklake__subcmd__help,support)
                cmd="rocklake__subcmd__help__subcmd__support"
                ;;
            rocklake__subcmd__help,sweep-orphans)
                cmd="rocklake__subcmd__help__subcmd__sweep__subcmd__orphans"
                ;;
            rocklake__subcmd__help,tune)
                cmd="rocklake__subcmd__help__subcmd__tune"
                ;;
            rocklake__subcmd__help,verify)
                cmd="rocklake__subcmd__help__subcmd__verify"
                ;;
            rocklake__subcmd__help,warmup)
                cmd="rocklake__subcmd__help__subcmd__warmup"
                ;;
            rocklake__subcmd__help__subcmd__backup,create)
                cmd="rocklake__subcmd__help__subcmd__backup__subcmd__create"
                ;;
            rocklake__subcmd__help__subcmd__backup,inspect)
                cmd="rocklake__subcmd__help__subcmd__backup__subcmd__inspect"
                ;;
            rocklake__subcmd__help__subcmd__capacity,report)
                cmd="rocklake__subcmd__help__subcmd__capacity__subcmd__report"
                ;;
            rocklake__subcmd__help__subcmd__catalog,backup)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__backup"
                ;;
            rocklake__subcmd__help__subcmd__catalog,backup-set)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set"
                ;;
            rocklake__subcmd__help__subcmd__catalog,checkpoint)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint"
                ;;
            rocklake__subcmd__help__subcmd__catalog,excise)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__excise"
                ;;
            rocklake__subcmd__help__subcmd__catalog,export)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__export"
                ;;
            rocklake__subcmd__help__subcmd__catalog,export-catalog)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__export__subcmd__catalog"
                ;;
            rocklake__subcmd__help__subcmd__catalog,gc)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__gc"
                ;;
            rocklake__subcmd__help__subcmd__catalog,import)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__import"
                ;;
            rocklake__subcmd__help__subcmd__catalog,jobs)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__jobs"
                ;;
            rocklake__subcmd__help__subcmd__catalog,maintenance)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance"
                ;;
            rocklake__subcmd__help__subcmd__catalog,migrate)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__migrate"
                ;;
            rocklake__subcmd__help__subcmd__catalog,recovery)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__recovery"
                ;;
            rocklake__subcmd__help__subcmd__catalog,repair)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__repair"
                ;;
            rocklake__subcmd__help__subcmd__catalog,restore)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__restore"
                ;;
            rocklake__subcmd__help__subcmd__catalog,verify)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__verify"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__backup,create)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__create"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__backup,inspect)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__inspect"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set,apply)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__apply"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set,create)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__create"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set,inspect)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__inspect"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set,plan)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__plan"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint,create)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__create"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint,list)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__list"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint,pin)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__pin"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint,pins)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__pins"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint,restore)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__restore"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint,unpin)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__unpin"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__excise,apply)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__excise__subcmd__apply"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__excise,plan)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__excise__subcmd__plan"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__gc,apply)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__gc__subcmd__apply"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__gc,plan)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__gc__subcmd__plan"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__jobs,cancel)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__jobs__subcmd__cancel"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__jobs,list)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__jobs__subcmd__list"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__jobs,resume)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__jobs__subcmd__resume"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__jobs,status)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__jobs__subcmd__status"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance,list)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance__subcmd__list"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance,remove)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance__subcmd__remove"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance,run)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance__subcmd__run"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance,schedule)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance__subcmd__schedule"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__recovery,report)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__recovery__subcmd__report"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__restore,apply)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__restore__subcmd__apply"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__restore,plan)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__restore__subcmd__plan"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__verify,catalog)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__verify__subcmd__catalog"
                ;;
            rocklake__subcmd__help__subcmd__catalog__subcmd__verify,data-files)
                cmd="rocklake__subcmd__help__subcmd__catalog__subcmd__verify__subcmd__data__subcmd__files"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,activate)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__activate"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,create)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__create"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,disable)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__disable"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,enable)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__enable"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,list)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__list"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,promote)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__promote"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,register)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__register"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,remove)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__remove"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,rename)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__rename"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,set-mode)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__set__subcmd__mode"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,status)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__status"
                ;;
            rocklake__subcmd__help__subcmd__catalogs,validate)
                cmd="rocklake__subcmd__help__subcmd__catalogs__subcmd__validate"
                ;;
            rocklake__subcmd__help__subcmd__checkpoint,create)
                cmd="rocklake__subcmd__help__subcmd__checkpoint__subcmd__create"
                ;;
            rocklake__subcmd__help__subcmd__checkpoint,list)
                cmd="rocklake__subcmd__help__subcmd__checkpoint__subcmd__list"
                ;;
            rocklake__subcmd__help__subcmd__checkpoint,pin)
                cmd="rocklake__subcmd__help__subcmd__checkpoint__subcmd__pin"
                ;;
            rocklake__subcmd__help__subcmd__checkpoint,pins)
                cmd="rocklake__subcmd__help__subcmd__checkpoint__subcmd__pins"
                ;;
            rocklake__subcmd__help__subcmd__checkpoint,restore)
                cmd="rocklake__subcmd__help__subcmd__checkpoint__subcmd__restore"
                ;;
            rocklake__subcmd__help__subcmd__checkpoint,unpin)
                cmd="rocklake__subcmd__help__subcmd__checkpoint__subcmd__unpin"
                ;;
            rocklake__subcmd__help__subcmd__config,check)
                cmd="rocklake__subcmd__help__subcmd__config__subcmd__check"
                ;;
            rocklake__subcmd__help__subcmd__config,example)
                cmd="rocklake__subcmd__help__subcmd__config__subcmd__example"
                ;;
            rocklake__subcmd__help__subcmd__corpus,diff)
                cmd="rocklake__subcmd__help__subcmd__corpus__subcmd__diff"
                ;;
            rocklake__subcmd__help__subcmd__corpus,validate)
                cmd="rocklake__subcmd__help__subcmd__corpus__subcmd__validate"
                ;;
            rocklake__subcmd__help__subcmd__debug,corpus)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__corpus"
                ;;
            rocklake__subcmd__help__subcmd__debug,diagnose)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__diagnose"
                ;;
            rocklake__subcmd__help__subcmd__debug,inspect)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__inspect"
                ;;
            rocklake__subcmd__help__subcmd__debug,migrate-from-ducklake)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__migrate__subcmd__from__subcmd__ducklake"
                ;;
            rocklake__subcmd__help__subcmd__debug,pg-migrate)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__pg__subcmd__migrate"
                ;;
            rocklake__subcmd__help__subcmd__debug,rebuild)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__rebuild"
                ;;
            rocklake__subcmd__help__subcmd__debug,sweep-orphans)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__sweep__subcmd__orphans"
                ;;
            rocklake__subcmd__help__subcmd__debug,tune)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__tune"
                ;;
            rocklake__subcmd__help__subcmd__debug,warmup)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__warmup"
                ;;
            rocklake__subcmd__help__subcmd__debug__subcmd__corpus,diff)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__corpus__subcmd__diff"
                ;;
            rocklake__subcmd__help__subcmd__debug__subcmd__corpus,validate)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__corpus__subcmd__validate"
                ;;
            rocklake__subcmd__help__subcmd__debug__subcmd__inspect,api-costs)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__inspect__subcmd__api__subcmd__costs"
                ;;
            rocklake__subcmd__help__subcmd__debug__subcmd__inspect,cache-utilization)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__inspect__subcmd__cache__subcmd__utilization"
                ;;
            rocklake__subcmd__help__subcmd__debug__subcmd__inspect,snapshot)
                cmd="rocklake__subcmd__help__subcmd__debug__subcmd__inspect__subcmd__snapshot"
                ;;
            rocklake__subcmd__help__subcmd__excise,apply)
                cmd="rocklake__subcmd__help__subcmd__excise__subcmd__apply"
                ;;
            rocklake__subcmd__help__subcmd__excise,plan)
                cmd="rocklake__subcmd__help__subcmd__excise__subcmd__plan"
                ;;
            rocklake__subcmd__help__subcmd__gc,apply)
                cmd="rocklake__subcmd__help__subcmd__gc__subcmd__apply"
                ;;
            rocklake__subcmd__help__subcmd__gc,plan)
                cmd="rocklake__subcmd__help__subcmd__gc__subcmd__plan"
                ;;
            rocklake__subcmd__help__subcmd__inspect,api-costs)
                cmd="rocklake__subcmd__help__subcmd__inspect__subcmd__api__subcmd__costs"
                ;;
            rocklake__subcmd__help__subcmd__inspect,cache-utilization)
                cmd="rocklake__subcmd__help__subcmd__inspect__subcmd__cache__subcmd__utilization"
                ;;
            rocklake__subcmd__help__subcmd__inspect,snapshot)
                cmd="rocklake__subcmd__help__subcmd__inspect__subcmd__snapshot"
                ;;
            rocklake__subcmd__help__subcmd__registry,backup)
                cmd="rocklake__subcmd__help__subcmd__registry__subcmd__backup"
                ;;
            rocklake__subcmd__help__subcmd__registry,init)
                cmd="rocklake__subcmd__help__subcmd__registry__subcmd__init"
                ;;
            rocklake__subcmd__help__subcmd__registry,migrate-static)
                cmd="rocklake__subcmd__help__subcmd__registry__subcmd__migrate__subcmd__static"
                ;;
            rocklake__subcmd__help__subcmd__registry,register-node)
                cmd="rocklake__subcmd__help__subcmd__registry__subcmd__register__subcmd__node"
                ;;
            rocklake__subcmd__help__subcmd__registry,renew-node)
                cmd="rocklake__subcmd__help__subcmd__registry__subcmd__renew__subcmd__node"
                ;;
            rocklake__subcmd__help__subcmd__registry,restore)
                cmd="rocklake__subcmd__help__subcmd__registry__subcmd__restore"
                ;;
            rocklake__subcmd__help__subcmd__registry,status)
                cmd="rocklake__subcmd__help__subcmd__registry__subcmd__status"
                ;;
            rocklake__subcmd__help__subcmd__registry,verify)
                cmd="rocklake__subcmd__help__subcmd__registry__subcmd__verify"
                ;;
            rocklake__subcmd__help__subcmd__restore,apply)
                cmd="rocklake__subcmd__help__subcmd__restore__subcmd__apply"
                ;;
            rocklake__subcmd__help__subcmd__restore,plan)
                cmd="rocklake__subcmd__help__subcmd__restore__subcmd__plan"
                ;;
            rocklake__subcmd__help__subcmd__support,bundle)
                cmd="rocklake__subcmd__help__subcmd__support__subcmd__bundle"
                ;;
            rocklake__subcmd__help__subcmd__verify,catalog)
                cmd="rocklake__subcmd__help__subcmd__verify__subcmd__catalog"
                ;;
            rocklake__subcmd__help__subcmd__verify,data-files)
                cmd="rocklake__subcmd__help__subcmd__verify__subcmd__data__subcmd__files"
                ;;
            rocklake__subcmd__inspect,api-costs)
                cmd="rocklake__subcmd__inspect__subcmd__api__subcmd__costs"
                ;;
            rocklake__subcmd__inspect,cache-utilization)
                cmd="rocklake__subcmd__inspect__subcmd__cache__subcmd__utilization"
                ;;
            rocklake__subcmd__inspect,help)
                cmd="rocklake__subcmd__inspect__subcmd__help"
                ;;
            rocklake__subcmd__inspect,snapshot)
                cmd="rocklake__subcmd__inspect__subcmd__snapshot"
                ;;
            rocklake__subcmd__inspect__subcmd__help,api-costs)
                cmd="rocklake__subcmd__inspect__subcmd__help__subcmd__api__subcmd__costs"
                ;;
            rocklake__subcmd__inspect__subcmd__help,cache-utilization)
                cmd="rocklake__subcmd__inspect__subcmd__help__subcmd__cache__subcmd__utilization"
                ;;
            rocklake__subcmd__inspect__subcmd__help,help)
                cmd="rocklake__subcmd__inspect__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__inspect__subcmd__help,snapshot)
                cmd="rocklake__subcmd__inspect__subcmd__help__subcmd__snapshot"
                ;;
            rocklake__subcmd__registry,backup)
                cmd="rocklake__subcmd__registry__subcmd__backup"
                ;;
            rocklake__subcmd__registry,help)
                cmd="rocklake__subcmd__registry__subcmd__help"
                ;;
            rocklake__subcmd__registry,init)
                cmd="rocklake__subcmd__registry__subcmd__init"
                ;;
            rocklake__subcmd__registry,migrate-static)
                cmd="rocklake__subcmd__registry__subcmd__migrate__subcmd__static"
                ;;
            rocklake__subcmd__registry,register-node)
                cmd="rocklake__subcmd__registry__subcmd__register__subcmd__node"
                ;;
            rocklake__subcmd__registry,renew-node)
                cmd="rocklake__subcmd__registry__subcmd__renew__subcmd__node"
                ;;
            rocklake__subcmd__registry,restore)
                cmd="rocklake__subcmd__registry__subcmd__restore"
                ;;
            rocklake__subcmd__registry,status)
                cmd="rocklake__subcmd__registry__subcmd__status"
                ;;
            rocklake__subcmd__registry,verify)
                cmd="rocklake__subcmd__registry__subcmd__verify"
                ;;
            rocklake__subcmd__registry__subcmd__help,backup)
                cmd="rocklake__subcmd__registry__subcmd__help__subcmd__backup"
                ;;
            rocklake__subcmd__registry__subcmd__help,help)
                cmd="rocklake__subcmd__registry__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__registry__subcmd__help,init)
                cmd="rocklake__subcmd__registry__subcmd__help__subcmd__init"
                ;;
            rocklake__subcmd__registry__subcmd__help,migrate-static)
                cmd="rocklake__subcmd__registry__subcmd__help__subcmd__migrate__subcmd__static"
                ;;
            rocklake__subcmd__registry__subcmd__help,register-node)
                cmd="rocklake__subcmd__registry__subcmd__help__subcmd__register__subcmd__node"
                ;;
            rocklake__subcmd__registry__subcmd__help,renew-node)
                cmd="rocklake__subcmd__registry__subcmd__help__subcmd__renew__subcmd__node"
                ;;
            rocklake__subcmd__registry__subcmd__help,restore)
                cmd="rocklake__subcmd__registry__subcmd__help__subcmd__restore"
                ;;
            rocklake__subcmd__registry__subcmd__help,status)
                cmd="rocklake__subcmd__registry__subcmd__help__subcmd__status"
                ;;
            rocklake__subcmd__registry__subcmd__help,verify)
                cmd="rocklake__subcmd__registry__subcmd__help__subcmd__verify"
                ;;
            rocklake__subcmd__restore,apply)
                cmd="rocklake__subcmd__restore__subcmd__apply"
                ;;
            rocklake__subcmd__restore,help)
                cmd="rocklake__subcmd__restore__subcmd__help"
                ;;
            rocklake__subcmd__restore,plan)
                cmd="rocklake__subcmd__restore__subcmd__plan"
                ;;
            rocklake__subcmd__restore__subcmd__help,apply)
                cmd="rocklake__subcmd__restore__subcmd__help__subcmd__apply"
                ;;
            rocklake__subcmd__restore__subcmd__help,help)
                cmd="rocklake__subcmd__restore__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__restore__subcmd__help,plan)
                cmd="rocklake__subcmd__restore__subcmd__help__subcmd__plan"
                ;;
            rocklake__subcmd__support,bundle)
                cmd="rocklake__subcmd__support__subcmd__bundle"
                ;;
            rocklake__subcmd__support,help)
                cmd="rocklake__subcmd__support__subcmd__help"
                ;;
            rocklake__subcmd__support__subcmd__help,bundle)
                cmd="rocklake__subcmd__support__subcmd__help__subcmd__bundle"
                ;;
            rocklake__subcmd__support__subcmd__help,help)
                cmd="rocklake__subcmd__support__subcmd__help__subcmd__help"
                ;;
            rocklake__subcmd__verify,catalog)
                cmd="rocklake__subcmd__verify__subcmd__catalog"
                ;;
            rocklake__subcmd__verify,data-files)
                cmd="rocklake__subcmd__verify__subcmd__data__subcmd__files"
                ;;
            rocklake__subcmd__verify,help)
                cmd="rocklake__subcmd__verify__subcmd__help"
                ;;
            rocklake__subcmd__verify__subcmd__help,catalog)
                cmd="rocklake__subcmd__verify__subcmd__help__subcmd__catalog"
                ;;
            rocklake__subcmd__verify__subcmd__help,data-files)
                cmd="rocklake__subcmd__verify__subcmd__help__subcmd__data__subcmd__files"
                ;;
            rocklake__subcmd__verify__subcmd__help,help)
                cmd="rocklake__subcmd__verify__subcmd__help__subcmd__help"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        rocklake)
            opts="-h -V --config --help --version serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__backup)
            opts="-h --config --help create inspect help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__backup__subcmd__create)
            opts="-c -h --catalog --out --snapshot-id --data-root --include-data --verify-data --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --out)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --snapshot-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__backup__subcmd__help)
            opts="create inspect help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__backup__subcmd__help__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__backup__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__backup__subcmd__help__subcmd__inspect)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__backup__subcmd__inspect)
            opts="-h --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__capacity)
            opts="-h --config --help report help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__capacity__subcmd__help)
            opts="report help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__capacity__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__capacity__subcmd__help__subcmd__report)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__capacity__subcmd__report)
            opts="-c -h --catalog --output --pricing-file --read-ops-per-second --write-ops-per-second --list-ops-per-second --delete-ops-per-second --read-bytes-per-second --write-bytes-per-second --catalog-bytes --cache-size-mb --max-sessions --max-active-scans --evidence-profile --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --pricing-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --read-ops-per-second)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --write-ops-per-second)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --list-ops-per-second)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --delete-ops-per-second)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --read-bytes-per-second)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --write-bytes-per-second)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cache-size-mb)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-sessions)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-active-scans)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --evidence-profile)
                    COMPREPLY=($(compgen -W "small medium large" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog)
            opts="-h --config --help backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup)
            opts="-h --config --help create inspect help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set)
            opts="-h --config --help create inspect plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__apply)
            opts="-h --input --registry --catalog-root --data-root --overwrite-token --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --input)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --overwrite-token)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__create)
            opts="-h --registry --output --catalog-id --include-data --verify-data --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help)
            opts="create inspect plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__inspect)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__help__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__inspect)
            opts="-h --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__plan)
            opts="-h --input --registry --catalog-root --data-root --overwrite-token --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --input)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --overwrite-token)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__create)
            opts="-c -h --catalog --out --snapshot-id --data-root --include-data --verify-data --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --out)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --snapshot-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__help)
            opts="create inspect help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__help__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__help__subcmd__inspect)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__backup__subcmd__inspect)
            opts="-h --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint)
            opts="-h --config --help create list restore pin unpin pins help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__create)
            opts="-c -h --catalog --label --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --label)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help)
            opts="create list restore pin unpin pins help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__pin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__pins)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__restore)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__help__subcmd__unpin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__list)
            opts="-c -h --catalog --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__pin)
            opts="-c -h --catalog --name --snapshot --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --snapshot)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__pins)
            opts="-c -h --catalog --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__restore)
            opts="-c -h --catalog --id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__checkpoint__subcmd__unpin)
            opts="-c -h --catalog --name --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__excise)
            opts="-h --config --help plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__excise__subcmd__apply)
            opts="-c -h --catalog --before --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --before)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__excise__subcmd__help)
            opts="plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__excise__subcmd__help__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__excise__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__excise__subcmd__help__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__excise__subcmd__plan)
            opts="-c -h --catalog --before --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --before)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__export)
            opts="-c -h --catalog --output --snapshot-id --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --snapshot-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__export__subcmd__catalog)
            opts="-c -h --catalog --out --at-snapshot --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --out)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --at-snapshot)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__gc)
            opts="-h --config --help plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__gc__subcmd__apply)
            opts="-c -h --catalog --retention-days --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --retention-days)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__gc__subcmd__help)
            opts="plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__gc__subcmd__help__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__gc__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__gc__subcmd__help__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__gc__subcmd__plan)
            opts="-c -h --catalog --retention-days --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --retention-days)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help)
            opts="backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__backup)
            opts="create inspect"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set)
            opts="create inspect plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set__subcmd__inspect)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__set__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__backup__subcmd__inspect)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint)
            opts="create list restore pin unpin pins"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__pin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__pins)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__restore)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__checkpoint__subcmd__unpin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__excise)
            opts="plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__excise__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__excise__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__export)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__export__subcmd__catalog)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__gc)
            opts="plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__gc__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__gc__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__import)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__jobs)
            opts="list status cancel resume"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__jobs__subcmd__cancel)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__jobs__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__jobs__subcmd__resume)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__jobs__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance)
            opts="schedule list remove run"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance__subcmd__remove)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance__subcmd__run)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__maintenance__subcmd__schedule)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__migrate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__recovery)
            opts="report"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__recovery__subcmd__report)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__repair)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__restore)
            opts="plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__restore__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__restore__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__verify)
            opts="catalog data-files"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__verify__subcmd__catalog)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__help__subcmd__verify__subcmd__data__subcmd__files)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__import)
            opts="-c -h --catalog --input --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --input)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs)
            opts="-h --config --help list status cancel resume help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__cancel)
            opts="-c -h --catalog --id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__help)
            opts="list status cancel resume help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__cancel)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__resume)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__list)
            opts="-c -h --catalog --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__resume)
            opts="-c -h --catalog --id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__jobs__subcmd__status)
            opts="-c -h --catalog --id --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance)
            opts="-h --config --help schedule list remove run help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help)
            opts="schedule list remove run help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__remove)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__run)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__help__subcmd__schedule)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__list)
            opts="-c -h --catalog --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__remove)
            opts="-c -h --catalog --id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__run)
            opts="-c -h --catalog --now-ms --limit --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --now-ms)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__maintenance__subcmd__schedule)
            opts="-c -h --catalog --id --task --interval-seconds --next-run-at-ms --window-start-minute --window-end-minute --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --task)
                    COMPREPLY=($(compgen -W "backup verification retention checkpoint orphan-sweep" -- "${cur}"))
                    return 0
                    ;;
                --interval-seconds)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --next-run-at-ms)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --window-start-minute)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --window-end-minute)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__migrate)
            opts="-c -h --catalog --dry-run --apply --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__recovery)
            opts="-h --config --help report help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__recovery__subcmd__help)
            opts="report help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__recovery__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__recovery__subcmd__help__subcmd__report)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__recovery__subcmd__report)
            opts="-h --drill --started-at --rpo-seconds --rto-seconds --verified --details --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --drill)
                    COMPREPLY=($(compgen -W "lost-process lost-registry-prefix accidental-route-deletion damaged-catalog-prefix lost-credentials region-restore" -- "${cur}"))
                    return 0
                    ;;
                --started-at)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --rpo-seconds)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --rto-seconds)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --details)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__repair)
            opts="-c -h --catalog --dry-run --apply --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__restore)
            opts="-h --config --help plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__restore__subcmd__apply)
            opts="-c -h --backup --catalog --overwrite --overwrite-token --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --backup)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --overwrite-token)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__restore__subcmd__help)
            opts="plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__restore__subcmd__help__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__restore__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__restore__subcmd__help__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__restore__subcmd__plan)
            opts="-c -h --backup --catalog --overwrite --overwrite-token --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --backup)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --overwrite-token)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__verify)
            opts="-h --config --help catalog data-files help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__verify__subcmd__catalog)
            opts="-c -h --catalog --output --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__verify__subcmd__data__subcmd__files)
            opts="-c -h --catalog --output --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__verify__subcmd__help)
            opts="catalog data-files help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__verify__subcmd__help__subcmd__catalog)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__verify__subcmd__help__subcmd__data__subcmd__files)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalog__subcmd__verify__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs)
            opts="-h --config --help validate list status create register promote activate rename set-mode disable enable remove help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__activate)
            opts="-h --registry --id --node-id --assignment-generation --writer-epoch --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --node-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --assignment-generation)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --writer-epoch)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__create)
            opts="-h --registry --id --alias --catalog --data --mode --credential-provider --policy-reference --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --alias)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --mode)
                    COMPREPLY=($(compgen -W "read-write read-only" -- "${cur}"))
                    return 0
                    ;;
                --credential-provider)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --policy-reference)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__disable)
            opts="-h --registry --id --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__enable)
            opts="-h --registry --id --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help)
            opts="validate list status create register promote activate rename set-mode disable enable remove help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__activate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__disable)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__enable)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__promote)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__register)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__remove)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__rename)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__set__subcmd__mode)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__help__subcmd__validate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__list)
            opts="-h --registry --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__promote)
            opts="-h --registry --id --node-id --endpoint --expected-generation --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --node-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --endpoint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --expected-generation)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__register)
            opts="-h --registry --id --alias --catalog --data --mode --credential-provider --policy-reference --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --alias)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --mode)
                    COMPREPLY=($(compgen -W "read-write read-only" -- "${cur}"))
                    return 0
                    ;;
                --credential-provider)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --policy-reference)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__remove)
            opts="-h --registry --id --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__rename)
            opts="-h --registry --id --alias --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --alias)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__set__subcmd__mode)
            opts="-h --registry --id --mode --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --mode)
                    COMPREPLY=($(compgen -W "read-write read-only" -- "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__status)
            opts="-h --registry --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__catalogs__subcmd__validate)
            opts="-h --registry --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint)
            opts="-h --config --help create list restore pin unpin pins help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__create)
            opts="-c -h --catalog --label --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --label)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__help)
            opts="create list restore pin unpin pins help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__help__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__help__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__help__subcmd__pin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__help__subcmd__pins)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__help__subcmd__restore)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__help__subcmd__unpin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__list)
            opts="-c -h --catalog --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__pin)
            opts="-c -h --catalog --name --snapshot --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --snapshot)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__pins)
            opts="-c -h --catalog --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__restore)
            opts="-c -h --catalog --id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__checkpoint__subcmd__unpin)
            opts="-c -h --catalog --name --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__completions)
            opts="-h --config --help bash elvish fish powershell zsh"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__config)
            opts="-h --config --help check example help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__config__subcmd__check)
            opts="-h --file --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__config__subcmd__example)
            opts="-h --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__config__subcmd__help)
            opts="check example help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__config__subcmd__help__subcmd__check)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__config__subcmd__help__subcmd__example)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__config__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__corpus)
            opts="-h --config --help diff validate help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__corpus__subcmd__diff)
            opts="-h --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__corpus__subcmd__help)
            opts="diff validate help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__corpus__subcmd__help__subcmd__diff)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__corpus__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__corpus__subcmd__help__subcmd__validate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__corpus__subcmd__validate)
            opts="-c -h --catalog --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug)
            opts="-h --config --help diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__corpus)
            opts="-h --config --help diff validate help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__corpus__subcmd__diff)
            opts="-h --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__corpus__subcmd__help)
            opts="diff validate help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__corpus__subcmd__help__subcmd__diff)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__corpus__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__corpus__subcmd__help__subcmd__validate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__corpus__subcmd__validate)
            opts="-c -h --catalog --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__diagnose)
            opts="-c -h --catalog --json --output --data-root --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help)
            opts="diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__corpus)
            opts="diff validate"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__corpus__subcmd__diff)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__corpus__subcmd__validate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__diagnose)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__inspect)
            opts="snapshot api-costs cache-utilization"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__inspect__subcmd__api__subcmd__costs)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__inspect__subcmd__cache__subcmd__utilization)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__inspect__subcmd__snapshot)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__migrate__subcmd__from__subcmd__ducklake)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__pg__subcmd__migrate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__rebuild)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__sweep__subcmd__orphans)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__tune)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__help__subcmd__warmup)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__inspect)
            opts="-h --config --help snapshot api-costs cache-utilization help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__inspect__subcmd__api__subcmd__costs)
            opts="-c -h --catalog --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__inspect__subcmd__cache__subcmd__utilization)
            opts="-c -h --catalog --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__inspect__subcmd__help)
            opts="snapshot api-costs cache-utilization help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__inspect__subcmd__help__subcmd__api__subcmd__costs)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__inspect__subcmd__help__subcmd__cache__subcmd__utilization)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__inspect__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__inspect__subcmd__help__subcmd__snapshot)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__inspect__subcmd__snapshot)
            opts="-c -h --catalog --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__migrate__subcmd__from__subcmd__ducklake)
            opts="-c -h --source --catalog --dry-run --accept-version --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --source)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --accept-version)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__pg__subcmd__migrate)
            opts="-h --input --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --input)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__rebuild)
            opts="-c -h --catalog --data-root --s3-endpoint --s3-path-style --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --s3-endpoint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__sweep__subcmd__orphans)
            opts="-c -h --catalog --data-root --grace-period-hours --apply --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --grace-period-hours)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__tune)
            opts="-c -h --catalog --target-cost-usd --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --target-cost-usd)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__debug__subcmd__warmup)
            opts="-c -h --catalog --tables --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tables)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__diagnose)
            opts="-c -h --catalog --json --output --data-root --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__doctor)
            opts="-c -h --catalog --mode --bind --tls-cert --tls-key --auth-user --auth-verifier-file --encryption-key --encryption-key-file --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --mode)
                    COMPREPLY=($(compgen -W "writer reader" -- "${cur}"))
                    return 0
                    ;;
                --bind)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-cert)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --auth-user)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --auth-verifier-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --encryption-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --encryption-key-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__excise)
            opts="-h --config --help plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__excise__subcmd__apply)
            opts="-c -h --catalog --before --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --before)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__excise__subcmd__help)
            opts="plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__excise__subcmd__help__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__excise__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__excise__subcmd__help__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__excise__subcmd__plan)
            opts="-c -h --catalog --before --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --before)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__export)
            opts="-c -h --catalog --output --snapshot-id --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --snapshot-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__export__subcmd__catalog)
            opts="-c -h --catalog --out --at-snapshot --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --out)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --at-snapshot)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__gc)
            opts="-h --config --help plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__gc__subcmd__apply)
            opts="-c -h --catalog --retention-days --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --retention-days)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__gc__subcmd__help)
            opts="plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__gc__subcmd__help__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__gc__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__gc__subcmd__help__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__gc__subcmd__plan)
            opts="-c -h --catalog --retention-days --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --retention-days)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help)
            opts="serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__backup)
            opts="create inspect"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__backup__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__backup__subcmd__inspect)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__capacity)
            opts="report"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__capacity__subcmd__report)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog)
            opts="backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__backup)
            opts="create inspect"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set)
            opts="create inspect plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__inspect)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__set__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__backup__subcmd__inspect)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint)
            opts="create list restore pin unpin pins"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__pin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__pins)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__restore)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__checkpoint__subcmd__unpin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__excise)
            opts="plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__excise__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__excise__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__export)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__export__subcmd__catalog)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__gc)
            opts="plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__gc__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__gc__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__import)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__jobs)
            opts="list status cancel resume"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__jobs__subcmd__cancel)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__jobs__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__jobs__subcmd__resume)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__jobs__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance)
            opts="schedule list remove run"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance__subcmd__remove)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance__subcmd__run)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__maintenance__subcmd__schedule)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__migrate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__recovery)
            opts="report"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__recovery__subcmd__report)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__repair)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__restore)
            opts="plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__restore__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__restore__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__verify)
            opts="catalog data-files"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__verify__subcmd__catalog)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalog__subcmd__verify__subcmd__data__subcmd__files)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs)
            opts="validate list status create register promote activate rename set-mode disable enable remove"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__activate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__disable)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__enable)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__promote)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__register)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__remove)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__rename)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__set__subcmd__mode)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__catalogs__subcmd__validate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__checkpoint)
            opts="create list restore pin unpin pins"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__checkpoint__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__checkpoint__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__checkpoint__subcmd__pin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__checkpoint__subcmd__pins)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__checkpoint__subcmd__restore)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__checkpoint__subcmd__unpin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__completions)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__config)
            opts="check example"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__config__subcmd__check)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__config__subcmd__example)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__corpus)
            opts="diff validate"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__corpus__subcmd__diff)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__corpus__subcmd__validate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug)
            opts="diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__corpus)
            opts="diff validate"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__corpus__subcmd__diff)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__corpus__subcmd__validate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__diagnose)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__inspect)
            opts="snapshot api-costs cache-utilization"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__inspect__subcmd__api__subcmd__costs)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__inspect__subcmd__cache__subcmd__utilization)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__inspect__subcmd__snapshot)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__migrate__subcmd__from__subcmd__ducklake)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__pg__subcmd__migrate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__rebuild)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__sweep__subcmd__orphans)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__tune)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__debug__subcmd__warmup)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__diagnose)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__doctor)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__excise)
            opts="plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__excise__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__excise__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__export)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__export__subcmd__catalog)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__gc)
            opts="plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__gc__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__gc__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__import)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__inspect)
            opts="snapshot api-costs cache-utilization"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__inspect__subcmd__api__subcmd__costs)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__inspect__subcmd__cache__subcmd__utilization)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__inspect__subcmd__snapshot)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__migrate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__migrate__subcmd__from__subcmd__ducklake)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__pg__subcmd__migrate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__rebuild)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__registry)
            opts="init status backup restore verify migrate-static register-node renew-node"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__registry__subcmd__backup)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__registry__subcmd__init)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__registry__subcmd__migrate__subcmd__static)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__registry__subcmd__register__subcmd__node)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__registry__subcmd__renew__subcmd__node)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__registry__subcmd__restore)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__registry__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__registry__subcmd__verify)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__repair)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__restore)
            opts="plan apply"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__restore__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__restore__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__serve)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__support)
            opts="bundle"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__support__subcmd__bundle)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__sweep__subcmd__orphans)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__tune)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__verify)
            opts="catalog data-files"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__verify__subcmd__catalog)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__verify__subcmd__data__subcmd__files)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__help__subcmd__warmup)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__import)
            opts="-c -h --catalog --input --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --input)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__inspect)
            opts="-h --config --help snapshot api-costs cache-utilization help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__inspect__subcmd__api__subcmd__costs)
            opts="-c -h --catalog --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__inspect__subcmd__cache__subcmd__utilization)
            opts="-c -h --catalog --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__inspect__subcmd__help)
            opts="snapshot api-costs cache-utilization help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__inspect__subcmd__help__subcmd__api__subcmd__costs)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__inspect__subcmd__help__subcmd__cache__subcmd__utilization)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__inspect__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__inspect__subcmd__help__subcmd__snapshot)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__inspect__subcmd__snapshot)
            opts="-c -h --catalog --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__migrate)
            opts="-c -h --catalog --dry-run --apply --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__migrate__subcmd__from__subcmd__ducklake)
            opts="-c -h --source --catalog --dry-run --accept-version --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --source)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --accept-version)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__pg__subcmd__migrate)
            opts="-h --input --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --input)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__rebuild)
            opts="-c -h --catalog --data-root --s3-endpoint --s3-path-style --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --s3-endpoint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry)
            opts="-h --config --help init status backup restore verify migrate-static register-node renew-node help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__backup)
            opts="-h --registry --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help)
            opts="init status backup restore verify migrate-static register-node renew-node help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help__subcmd__backup)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help__subcmd__init)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help__subcmd__migrate__subcmd__static)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help__subcmd__register__subcmd__node)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help__subcmd__renew__subcmd__node)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help__subcmd__restore)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help__subcmd__status)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__help__subcmd__verify)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__init)
            opts="-h --registry --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__migrate__subcmd__static)
            opts="-h --registry --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__register__subcmd__node)
            opts="-h --registry --node-id --endpoint --lease-seconds --lease-id --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --node-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --endpoint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --lease-seconds)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --lease-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__renew__subcmd__node)
            opts="-h --registry --node-id --lease-id --lease-seconds --request-id --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --node-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --lease-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --lease-seconds)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --request-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__restore)
            opts="-h --registry --input --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --input)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__status)
            opts="-h --registry --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__registry__subcmd__verify)
            opts="-h --registry --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --registry)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__repair)
            opts="-c -h --catalog --dry-run --apply --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__restore)
            opts="-h --config --help plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__restore__subcmd__apply)
            opts="-c -h --backup --catalog --overwrite --overwrite-token --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --backup)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --overwrite-token)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__restore__subcmd__help)
            opts="plan apply help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__restore__subcmd__help__subcmd__apply)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__restore__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__restore__subcmd__help__subcmd__plan)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__restore__subcmd__plan)
            opts="-c -h --backup --catalog --overwrite --overwrite-token --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --backup)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --overwrite-token)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__serve)
            opts="-c -b -h --catalog --bind --max-sessions --metrics-port --metrics-path --tls-cert --tls-key --tls-required --auth-user --auth-password --auth-password-file --auth-verifier-file --mode --read-only --cost-mode --s3-endpoint --s3-path-style --encryption-key --encryption-key-file --extension-schemas --otlp-endpoint --idle-connection-timeout --drain-timeout --max-active-scans --stream-queue-depth --max-buffered-rows --max-response-bytes --slow-operation-threshold-ms --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --bind)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -b)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-sessions)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --metrics-port)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --metrics-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-cert)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --auth-user)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --auth-password)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --auth-password-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --auth-verifier-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --mode)
                    COMPREPLY=($(compgen -W "writer reader" -- "${cur}"))
                    return 0
                    ;;
                --cost-mode)
                    COMPREPLY=($(compgen -W "conservative balanced latency" -- "${cur}"))
                    return 0
                    ;;
                --s3-endpoint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --encryption-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --encryption-key-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --extension-schemas)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --otlp-endpoint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idle-connection-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --drain-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-active-scans)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --stream-queue-depth)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-buffered-rows)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --max-response-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --slow-operation-threshold-ms)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__status)
            opts="-c -h --catalog --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__support)
            opts="-h --config --help bundle help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__support__subcmd__bundle)
            opts="-c -h --output --catalog --metrics-url --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --metrics-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__support__subcmd__help)
            opts="bundle help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__support__subcmd__help__subcmd__bundle)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__support__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__sweep__subcmd__orphans)
            opts="-c -h --catalog --data-root --grace-period-hours --apply --idempotency-key --output --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data-root)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --grace-period-hours)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__tune)
            opts="-c -h --catalog --target-cost-usd --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --target-cost-usd)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__verify)
            opts="-h --config --help catalog data-files help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__verify__subcmd__catalog)
            opts="-c -h --catalog --output --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__verify__subcmd__data__subcmd__files)
            opts="-c -h --catalog --output --idempotency-key --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --idempotency-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__verify__subcmd__help)
            opts="catalog data-files help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__verify__subcmd__help__subcmd__catalog)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__verify__subcmd__help__subcmd__data__subcmd__files)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__verify__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        rocklake__subcmd__warmup)
            opts="-c -h --catalog --tables --config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --catalog)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tables)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _rocklake -o nosort -o bashdefault -o default rocklake
else
    complete -F _rocklake -o bashdefault -o default rocklake
fi
