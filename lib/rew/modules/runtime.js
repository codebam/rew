const vm = require('vm');
const { compileFile } = require('./compiler');
const { prepareContext } = require('./context');
const { existsSync, readFileSync } = require('fs');
const { CONFIG_PATH } = require('../const/config_path');
const path = require('path');

const preScript = readFileSync(path.join(__dirname, '../const/pre-exec.js'), { encoding: 'utf-8' });

const exec = (module.exports.exec = function (code, context) {
	return vm.runInNewContext(code, vm.createContext(context), {
		filename: context.module.filepath,
		lineOffset: -1 - preScript.split('\n').length,
		displayErrors: true,
	});
});

module.exports.runPath = function runPath(filepath, options = {}, custom_context = {}) {
	if(filepath.endsWith('.js')) options.type = 'js';
	let { compiled_code, file } = compileFile(filepath, options);
	const context = prepareContext(custom_context, options, file.path, runPath);

	context.module.compiled = compiled_code;
	context.process.exit = (int) => process.exit(int);

	if(context.app){
		const p = path.join(CONFIG_PATH, context.app.config.manifest.package, 'app');
		if(existsSync(p) && context.app.path !== p){
			console.log("App with the same package name has been installed. Conflicts happened. \nTo fix this, change your app's package name or remove the app making the conflict.");
			return {
				context: { exports: null },
				returns: null
			}
		}
	}

	compiled_code = preScript+compiled_code;

	return {
		context,
		returns: exec(compiled_code, context),
	};
};
