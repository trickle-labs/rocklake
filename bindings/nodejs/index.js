// index.js — JavaScript entry point for @rocklake/client
// Loads the platform-specific napi binary.

const fs = require('node:fs');
const path = require('node:path');

function resolveAddon() {
	const rootAddon = path.join(__dirname, 'rocklake.node');
	if (fs.existsSync(rootAddon)) {
		return rootAddon;
	}

	const candidates = [
		path.join(__dirname, 'target', 'debug', 'deps', 'librocklake_node.dylib'),
		path.join(__dirname, 'target', 'debug', 'deps', 'librocklake_node.so'),
		path.join(__dirname, 'target', 'release', 'deps', 'librocklake_node.dylib'),
		path.join(__dirname, 'target', 'release', 'deps', 'librocklake_node.so'),
	];

	for (const candidate of candidates) {
		if (fs.existsSync(candidate)) {
			fs.copyFileSync(candidate, rootAddon);
			return rootAddon;
		}
	}

	return rootAddon;
}

const { Catalog } = require(resolveAddon());

module.exports = { Catalog };
