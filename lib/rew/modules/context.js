const defaultContext = require("../const/default");
const { execOptions } = require("../const/opt");
const { exportsFunction, pubFunction } = require("../functions/export");
const { imp } = require("../functions/import");
const { customRequire } = require("../functions/require");
const fsLib = require("../functions/fs");
const pathLib = require("../functions/path");
const execLib = require("../functions/exec");
const { findAppInfo } = require("../misc/findAppInfo");

let mainFile = "";
const isMainFile = (filepath) => filepath == mainFile;
module.exports.prepareContext = function (
  custom_context,
  options,
  filepath = "",
  runPath = () => {},
) {
  if (mainFile == "") mainFile = filepath;
  let context = {
    module: {
      exports: null,
      filepath,
      main: isMainFile(filepath),
      imports: [],
    },
    imports: {
      meta: {},
      assert: options.import ?? {},
    },
    app: findAppInfo(filepath),
    ...fsLib(filepath),
  };
  if (options.useContext) {
    context = {
      ...custom_context,
      ...context,
    };
  } else {
    context = {
      ...context,
      ...defaultContext,
      ...pathLib(filepath),
      ...execLib(filepath),
      ...custom_context,
    };
  }
  if (!context.process)
    context.process = {
      argv: options.argv || process.argv,
      target: {
        on: (event, listener) => process.on(event, listener),
        off: (event, listener) => process.off(event, listener),
        emit: (event, code) => process.emit(event, code),
      },
      env: process.env,
      cwd: () => process.cwd(),
      arch: process.arch,
    };

  context.global = context;
  context.imports.assert = options.import ?? {};
  context.imp = imp(runPath, context);
  context.require = (package) => {
    try {
      const search = execOptions.nativeRequire || package.startsWith("node:")
        ? require(
            package.startsWith("node:")
              ? package.split("node:")[1]
              : package,
          )
        : customRequire(package, filepath);
        if(!search) throw new Error("Module " + package + " not found");
        return search;
    } catch (e) {
      throw e;
    }
  };
  context.inc = (package, asserts) => {
    try {
      if (package.startsWith("node:") || package.startsWith("pkg:"))
        throw new Error("");
      return context.imp(package, asserts);
    } catch (e) {
      return context.require(
        package.startsWith("pkg:") ? package.split("pkg:")[1] : package,
      );
    }
  };
  context.pub = pubFunction(context);
  context.exports = exportsFunction(context);

  if (
    context.module.main ||
    (options.fromMain == true && options.as == "main")
  ) {
    context.opt = {
      set: (key, value) => (execOptions[key] = value),
      get: (key) => execOptions[key],
      push: (key, value) => execOptions[key]?.push(value),
      pop: (key) => execOptions[key]?.pop(),
    };
  } else delete context.opt;
  return context;
};
