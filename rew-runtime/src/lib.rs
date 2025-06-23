use anyhow::{Context, Result};
use deno_core::{JsRuntime, RuntimeOptions, PollEventLoopOptions};
use deno_fs::{FileSystem, RealFs};
use deno_permissions::{
  AllowRunDescriptor, AllowRunDescriptorParseResult, DenyRunDescriptor, EnvDescriptor,
  EnvDescriptorParseError, FfiDescriptor, ImportDescriptor, NetDescriptor, NetDescriptorParseError,
  PathQueryDescriptor, PathResolveError, PermissionDescriptorParser, PermissionsContainer,
  ReadDescriptor, RunDescriptorParseError, RunQueryDescriptor, SysDescriptor,
  SysDescriptorParseError, WriteDescriptor,
};
use futures::stream::{self, StreamExt};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

// Re-export the extension modules
pub use rew_fs;
pub use rew_data;
pub use rew_os;
pub use rew_utils;

#[derive(Default)]
pub struct RuntimeArgs(pub Vec<String>);

#[derive(Default)]
struct RuntimeState {
  current_dir: PathBuf,
  args: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
  pub bundle_all: bool,
  pub entry_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct TestPermissionDescriptorParser;

impl TestPermissionDescriptorParser {
  fn join_path_with_root(&self, path: &str) -> PathBuf {
    if path.starts_with("C:\\") {
      PathBuf::from(path)
    } else {
      PathBuf::from("/").join(path)
    }
  }
}

impl PermissionDescriptorParser for TestPermissionDescriptorParser {
  fn parse_read_descriptor(&self, text: &str) -> Result<ReadDescriptor, PathResolveError> {
    Ok(ReadDescriptor(self.join_path_with_root(text)))
  }

  fn parse_write_descriptor(&self, text: &str) -> Result<WriteDescriptor, PathResolveError> {
    Ok(WriteDescriptor(self.join_path_with_root(text)))
  }

  fn parse_net_descriptor(&self, text: &str) -> Result<NetDescriptor, NetDescriptorParseError> {
    NetDescriptor::parse(text)
  }

  fn parse_import_descriptor(
    &self,
    text: &str,
  ) -> Result<ImportDescriptor, NetDescriptorParseError> {
    ImportDescriptor::parse(text)
  }

  fn parse_env_descriptor(&self, text: &str) -> Result<EnvDescriptor, EnvDescriptorParseError> {
    Ok(EnvDescriptor::new(text))
  }

  fn parse_sys_descriptor(&self, text: &str) -> Result<SysDescriptor, SysDescriptorParseError> {
    SysDescriptor::parse(text.to_string())
  }

  fn parse_allow_run_descriptor(
    &self,
    text: &str,
  ) -> Result<AllowRunDescriptorParseResult, RunDescriptorParseError> {
    Ok(AllowRunDescriptorParseResult::Descriptor(
      AllowRunDescriptor(self.join_path_with_root(text)),
    ))
  }

  fn parse_deny_run_descriptor(&self, text: &str) -> Result<DenyRunDescriptor, PathResolveError> {
    if text.contains("/") {
      Ok(DenyRunDescriptor::Path(self.join_path_with_root(text)))
    } else {
      Ok(DenyRunDescriptor::Name(text.to_string()))
    }
  }

  fn parse_ffi_descriptor(&self, text: &str) -> Result<FfiDescriptor, PathResolveError> {
    Ok(FfiDescriptor(self.join_path_with_root(text)))
  }

  fn parse_path_query(&self, path: &str) -> Result<PathQueryDescriptor, PathResolveError> {
    Ok(PathQueryDescriptor {
      resolved: self.join_path_with_root(path),
      requested: path.to_string(),
    })
  }

  fn parse_run_query(
    &self,
    requested: &str,
  ) -> Result<RunQueryDescriptor, RunDescriptorParseError> {
    RunQueryDescriptor::parse(requested).map_err(Into::into)
  }
}

pub fn get_rew_runtime(
  is_compiler: bool,
  is_main: bool,
  args: Option<Vec<String>>,
) -> Result<JsRuntime> {
  let mut extensions = vec![];

  // Add our custom extensions
  extensions.push(rew_fs::rew_fs::init());
  extensions.push(rew_data::rew_data::init());
  extensions.push(rew_os::rew_os::init());
  extensions.push(rew_utils::rew_utils::init());

  // Add external extensions
  extensions.extend(deno_webidl::deno_webidl::init());
  extensions.extend(deno_console::deno_console::init());
  extensions.extend(deno_url::deno_url::init());
  extensions.extend(deno_web::deno_web::init(
    deno_web::BlobStore::default(),
    None,
  ));
  extensions.extend(deno_ffi::deno_ffi::init::<deno_permissions::PermissionsContainer>());
  extensions.extend(deno_fs::deno_fs::init::<deno_permissions::PermissionsContainer>(
    std::rc::Rc::new(RealFs) as std::rc::Rc<dyn FileSystem>,
  ));
  extensions.extend(deno_io::deno_io::init(Some(deno_io::Stdio {
    stdin: deno_io::StdioPipe::inherit(),
    stderr: deno_io::StdioPipe::inherit(),
    stdout: deno_io::StdioPipe::inherit(),
  })));
  extensions.extend(deno_os::deno_os::init::<deno_permissions::PermissionsContainer>());
  extensions.extend(deno_process::deno_process::init::<deno_permissions::PermissionsContainer>());

  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions,
    is_main,
    ..Default::default()
  });

  let current_dir = std::env::current_dir()?;

  let state = RuntimeState {
    current_dir: current_dir.clone(),
    args: args.unwrap_or_default(),
  };

  runtime.op_state().borrow_mut().put(state);

  let permissions =
    PermissionsContainer::allow_all(std::sync::Arc::new(TestPermissionDescriptorParser));
  runtime.op_state().borrow_mut().put::<PermissionsContainer>(permissions);

  runtime.execute_script(
    "<setup>",
    r#"
globalThis._execVM = (namespace, fn) => {
  with(namespace){
  eval(`(${fn.toString()})()`);
  }
}
globalThis._evalVM = (string) => eval(string);
"#,
  )?;

  // Add runtime script here if needed
  // runtime.execute_script("<setup>", get_runtime_script())?;

  if is_compiler {
    // Add civet script here if needed
    // runtime.execute_script("<civet>", get_civet_script()).unwrap();
  }

  Ok(runtime)
}

pub fn is_js_executable(mod_id: &str) -> bool {
  matches!(
    mod_id.rsplit('.').next(),
    Some("ts" | "js" | "coffee" | "civet" | "rew")
  )
}

pub struct RewRuntime {
  pub runtime: JsRuntime,
  sourcemap: bool,
  inlinemap: bool,
  compile_options: Vec<String>,
}

impl RewRuntime {
  pub fn new(args: Option<Vec<String>>, jruntime: Option<JsRuntime>) -> Result<Self> {
    let runtime = jruntime.unwrap_or_else(|| get_rew_runtime(true, true, args).unwrap());

    Ok(Self {
      runtime,
      inlinemap: false,
      sourcemap: false,
      compile_options: vec![],
    })
  }

  pub fn resolve_includes_recursive_from<P: AsRef<Path>>(
    filepath: P,
  ) -> Result<Vec<(PathBuf, String, bool)>> {
    let filepath = filepath
      .as_ref()
      .canonicalize()
      .with_context(|| "Failed to resolve import".to_string())?;

    let import_re = Regex::new(r#"(?m)^\s*import\s+(?:[^;]*?\s+from\s+)?["']([^"']+)["']"#)
      .context("Invalid regex pattern")?;
    let external_re =
      Regex::new(r#"(?m)^\s*// external\s+['"]([^'"]+)['"]"#).context("Invalid regex pattern")?;

    let mut visited = HashSet::new();
    let mut result = Vec::new();

    fn visit_file(
      file_path: &Path,
      visited: &mut HashSet<PathBuf>,
      result: &mut Vec<(PathBuf, String, bool)>,
      preprocess_import: bool,
      import_re: &Regex,
      external_re: &Regex,
    ) -> Result<()> {
      if visited.contains(file_path) {
        return Ok(());
      }

      let mut should_preprocess = preprocess_import;
      let file_path_unf = file_path.to_str().unwrap_or("");
      let file_path_str = if file_path_unf.ends_with('!') {
        should_preprocess = true;
        &file_path_unf[0..file_path_unf.len() - 1]
      } else {
        file_path_unf
      };

      let is_brew_file = file_path
        .extension()
        .is_some_and(|ext| ext == "brew" || ext == "qrew");

      let real_content = if let Some(v) = rew_utils::VIRTUAL_FILES
        .lock()
        .unwrap()
        .iter()
        .find(|(p, _)| p == file_path_str)
      {
        v.1.clone()
      } else if file_path_str.starts_with("#") {
        "".to_string()
      } else {
        fs::read_to_string(file_path).with_context(|| format!("Failed to read {:?}", file_path))?
      };

      let content = if file_path_str.starts_with("#") {
        // Handle builtin modules here if needed
        real_content
      } else if is_brew_file {
        // Handle brew file decoding here if needed
        real_content
      } else {
        real_content
      };

      visited.insert(PathBuf::from(file_path_str));
      result.push((PathBuf::from(file_path_str), content.clone(), should_preprocess));

      let parent = file_path.parent().unwrap_or(Path::new("."));

      if is_brew_file || content.starts_with("\"no-compile\"") {
        for cap in external_re.captures_iter(&content) {
          let external_app_path = cap[1].to_string();
          let mut should_preprocess_import = false;
          let external_app = if external_app_path.ends_with('!') {
            should_preprocess_import = true;
            &external_app_path[0..external_app_path.len() - 1]
          } else {
            &external_app_path
          };

          // Handle external app resolution here
          visit_file(
            &PathBuf::from(external_app),
            visited,
            result,
            should_preprocess_import,
            import_re,
            external_re,
          )?;
        }
      } else {
        for cap in import_re.captures_iter(&content) {
          let relative_path_raw = cap[1].to_string();
          let mut should_preprocess_import = false;
          let relative_path = if relative_path_raw.ends_with('!') {
            should_preprocess_import = true;
            &relative_path_raw[0..relative_path_raw.len() - 1]
          } else {
            &relative_path_raw
          };

          if relative_path.starts_with("#") {
            let builtin_path = PathBuf::from(&relative_path);
            visit_file(
              &builtin_path,
              visited,
              result,
              should_preprocess_import,
              import_re,
              external_re,
            )?;
          } else {
            let included_path = parent
              .join(relative_path)
              .canonicalize()
              .with_context(|| "Failed to resolve import".to_string())?;

            visit_file(
              &included_path,
              visited,
              result,
              should_preprocess_import,
              import_re,
              external_re,
            )?;
          }
        }
      }

      Ok(())
    }

    visit_file(
      &filepath,
      &mut visited,
      &mut result,
      false,
      &import_re,
      &external_re,
    )?;
    Ok(result)
  }

  pub async fn prepare(
    &mut self,
    files: Vec<(PathBuf, String)>,
    entry: Option<&Path>,
  ) -> Result<String> {
    let mut module_wrappers = String::new();
    let mut entry_calls = Vec::new();

    let results = stream::iter(files.into_iter().map(|(path_original, source)| {
      let path = path_original.clone();
      let source = source.clone();
      async move {
        let res = tokio::task::spawn_blocking(move || {
          // Simple compilation - in real app you'd use the compiler
          Ok::<String, anyhow::Error>(source)
        }).await
        .map_err(|e| anyhow::anyhow!("Thread join error: {}", e))??;
        Ok::<(PathBuf, String), anyhow::Error>((path_original.clone(), res))
      }
    }))
    .buffer_unordered(16)
    .collect::<Vec<_>>()
    .await;

    let entry_regex = Regex::new(r#"//\s*entry\s*"([^"]+)""#).unwrap();
    for result in results {
      let (path, compiled) = result?;
      let mod_id = path
        .to_str()
        .unwrap_or("unknown")
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('"', "\\\"");

      if mod_id.starts_with('#') {
        module_wrappers.push_str(&format!(
          "(function(module){{\n{compiled}\n}})({{filename: \"{id}\"}});",
          id = mod_id,
          compiled = compiled
        ));
      } else if mod_id.ends_with(".brew") || mod_id.ends_with(".qrew") {
        module_wrappers.push_str(compiled.as_str());

        for cap in entry_regex.captures_iter(&compiled) {
          let entry_file = cap[1].to_string();
          entry_calls.push(format!(
            "rew.prototype.mod.prototype.get('{}');",
            entry_file.replace('\\', "\\\\")
          ));
        }
      } else if is_js_executable(&mod_id) {
        module_wrappers.push_str(&format!(
          r#"rew.prototype.mod.prototype.defineNew("{id}", {{
"{id}"(globalThis){{
with (globalThis) {{
  {compiled}
}}
return globalThis.module.exports;
}}          
}}, []);"#,
          id = mod_id,
          compiled = compiled
        ));
      } else {
        module_wrappers.push_str(&format!(
          r#"rew.prototype.mod.prototype.defineNew("{id}", function(globalThis){{
  return rew.prototype.mod.prototype.preprocess("{id}", `{compiled}`);
}}, []);"#,
          id = mod_id,
          compiled = compiled.replace("`", "\\`").replace("\\", "\\\\")
        ));
      }
    }

    if let Some(entry) = entry {
      let entry_mod_id = entry.to_str().unwrap_or("entry");
      let final_entry_id = entry_mod_id.to_string().replace('\\', "\\\\");
      if entry_calls.is_empty()
        && !final_entry_id.ends_with(".brew")
        && !final_entry_id.ends_with(".qrew")
      {
        entry_calls.push(format!(
          "rew.prototype.mod.prototype.get('{}');",
          final_entry_id
        ));
      }
    }

    for entry_call in entry_calls {
      module_wrappers.push_str(&format!("\n{}", entry_call));
    }

    Ok(module_wrappers.to_string())
  }

  pub async fn include_and_run(
    &mut self,
    files: Vec<(PathBuf, String)>,
    entry: &Path,
  ) -> Result<()> {
    let final_script = self.prepare(files, Some(entry)).await?;

    self.runtime.execute_script("<main>", final_script)?;
    self
      .runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;
    Ok(())
  }

  pub async fn run_file<P: AsRef<Path>>(&mut self, filepath: P) -> Result<()> {
    let filepath = filepath
      .as_ref()
      .canonicalize()
      .with_context(|| format!("Failed to resolve file path: {:?}", filepath.as_ref()))?;

    let files_with_flags = RewRuntime::resolve_includes_recursive_from(&filepath)?;

    let files: Vec<(PathBuf, String)> = files_with_flags
      .into_iter()
      .map(|(path, content, _)| (path, content))
      .collect();

    self.include_and_run(files, &filepath).await?;

    Ok(())
  }
}

impl Drop for RewRuntime {
  fn drop(&mut self) {}
}

