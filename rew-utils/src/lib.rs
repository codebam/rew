use deno_core::{extension, op2, OpState, CoreError};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng, distributions::Alphanumeric};
use std::hash::Hash;
use std::hash::Hasher;
use once_cell::sync::Lazy;
use std::sync::Mutex;

// Virtual files storage
pub static VIRTUAL_FILES: Lazy<Mutex<Vec<(String, String)>>> = Lazy::new(|| Mutex::new(vec![]));

/// Adds a virtual file to the runtime's virtual file storage.
pub fn add_virtual_file(path: &str, contents: &str) {
  let mut files = VIRTUAL_FILES.lock().unwrap();
  files.push((path.to_string(), contents.to_string()));
}

#[derive(Deserialize, Default)]
pub struct Base64DecodeOptions {
  pub as_string: bool,
}

#[derive(Default)]
struct RuntimeState {
    current_dir: PathBuf,
    args: Vec<String>,
}

extension!(
  rew_utils,
  ops = [
    op_get_args,
    op_to_base64,
    op_from_base64,
    op_find_app,
    op_yaml_to_string,
    op_string_to_yaml,
    op_app_loadconfig,
    op_fetch_env,
    op_dyn_imp,
    op_rand_from,
    op_vfile_set,
    op_vfile_get,
    op_gen_uid,
  ]
);

#[op2]
#[serde]
fn op_get_args(state: Rc<RefCell<OpState>>) -> Result<serde_json::Value, CoreError> {
  let state = state.borrow();
  let runtime_args = state.borrow::<RuntimeState>();
  Ok(serde_json::json!(runtime_args.args.clone()))
}

#[op2]
#[string]
fn op_to_base64(#[serde] data: serde_json::Value) -> Result<String, CoreError> {
  match data {
    serde_json::Value::String(text) => Ok(BASE64.encode(text.as_bytes())),
    serde_json::Value::Array(bytes) => {
      let buffer: Result<Vec<u8>, _> = bytes
        .iter()
        .map(|v| {
          if let serde_json::Value::Number(n) = v {
            n.as_u64().map(|n| n as u8).ok_or_else(|| {
              CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid byte value",
              ))
            })
          } else {
            Err(CoreError::Io(std::io::Error::new(
              std::io::ErrorKind::InvalidData,
              "Expected number in byte array",
            )))
          }
        })
        .collect();

      match buffer {
        Ok(bytes) => Ok(BASE64.encode(bytes)),
        Err(e) => Err(e),
      }
    }
    _ => Err(CoreError::Io(std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      "Expected string or array of bytes for base64 encoding",
    ))),
  }
}

#[op2]
#[serde]
fn op_from_base64(
  #[string] encoded: String,
  #[serde] options: Option<Base64DecodeOptions>,
) -> Result<serde_json::Value, CoreError> {
  let options = options.unwrap_or_default();

  let decoded = BASE64
    .decode(encoded.as_bytes())
    .map_err(|e| CoreError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

  if options.as_string {
    let text = String::from_utf8(decoded)
      .map_err(|e| CoreError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    Ok(serde_json::Value::String(text))
  } else {
    Ok(serde_json::Value::Array(
      decoded
        .into_iter()
        .map(|b| serde_json::Value::Number(b.into()))
        .collect(),
    ))
  }
}

#[op2]
#[string]
fn op_find_app(#[string] filepath: String, _: Rc<RefCell<OpState>>) -> Result<String, CoreError> {
  let current_file = Path::new(&filepath);

  // Simple implementation - in real app you'd use find_app_path from utils
  let app_path = find_app_path_simple(current_file);

  Ok(String::from(
    app_path.unwrap_or(PathBuf::from("")).to_str().unwrap(),
  ))
}

// Simple implementation of app path finding
fn find_app_path_simple(current_file: &Path) -> Option<PathBuf> {
  let mut path = current_file.to_path_buf();
  loop {
    if path.join("app.yaml").exists() {
      return Some(path);
    }
    if !path.pop() {
      break;
    }
  }
  None
}

#[op2]
#[string]
fn op_yaml_to_string(
  #[serde] data: serde_json::Value,
  _: Rc<RefCell<OpState>>,
) -> Result<String, CoreError> {
  let yaml = serde_yaml::to_string(&data)
    .map_err(|e| CoreError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

  Ok(yaml)
}

#[op2]
#[serde]
fn op_string_to_yaml(
  #[string] yaml_str: String,
  _: Rc<RefCell<OpState>>,
) -> Result<serde_json::Value, CoreError> {
  let value: serde_json::Value = serde_yaml::from_str(&yaml_str)
    .map_err(|e| CoreError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

  Ok(value)
}

#[op2]
#[serde]
fn op_app_loadconfig(
  #[string] app_path: String,
  _: Rc<RefCell<OpState>>,
) -> Result<serde_json::Value, CoreError> {
  let app_path = Path::new(&app_path);

  if !app_path.exists() {
    return Err(CoreError::Io(std::io::Error::new(
      std::io::ErrorKind::NotFound,
      format!("App path not found: {}", app_path.display()),
    )));
  }

  let config_path = app_path.join("app.yaml");

  if !config_path.exists() {
    return Err(CoreError::Io(std::io::Error::new(
      std::io::ErrorKind::NotFound,
      format!("App config not found: {}", config_path.display()),
    )));
  }

  let config_str = fs::read_to_string(&config_path)
    .map_err(|e| CoreError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

  let config: serde_json::Value = serde_yaml::from_str(&config_str)
    .map_err(|e| CoreError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

  Ok(config)
}

#[op2]
#[string]
fn op_fetch_env(_: Rc<RefCell<OpState>>) -> Result<String, CoreError> {
  let env_vars: HashMap<String, String> = std::env::vars().collect();
  let cwd = std::env::current_dir()
    .map_err(|e| CoreError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
    .to_string_lossy()
    .to_string();
  let exec_path = std::env::current_exe()
    .map_err(|e| CoreError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
    .to_string_lossy()
    .to_string();

  let result = serde_json::json!({
    "env": env_vars,
    "cwd": cwd,
    "execPath": exec_path,
    "tempDir": std::env::temp_dir(),
    "rewPath": "." // Simple fallback
  });

  serde_json::to_string(&result)
    .map_err(|e| CoreError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
}

#[op2(async, reentrant)]
#[serde]
async fn op_dyn_imp(
  #[string] current_file: String,
  #[string] file: String,
  _: Rc<RefCell<OpState>>,
) -> Result<serde_json::Value, CoreError> {
  let file_path = if current_file == "/" {
    Path::new(&file).to_path_buf()
  } else {
    let current_file_path = Path::new(&current_file);
    let base_dir = current_file_path.parent().unwrap_or(Path::new("."));
    base_dir.join(file)
  };

  // Simple implementation - in real app you'd create a new RewRuntime
  let fp = fs::canonicalize(&file_path)
    .map_err(|_| CoreError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "")))?;

  let content = fs::read_to_string(&fp)
    .map_err(|_| CoreError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "")))?;

  Ok(serde_json::json!(vec![
    fp.to_string_lossy().to_string(),
    content
  ]))
}

#[op2]
#[serde]
fn op_rand_from(
  #[bigint] min: usize,
  #[bigint] max: usize,
  #[string] seed: Option<String>,
) -> usize {
  let mut rng: Box<dyn RngCore> = match seed {
    Some(s) => {
      let mut hasher = std::collections::hash_map::DefaultHasher::new();
      s.hash(&mut hasher);
      Box::new(StdRng::seed_from_u64(hasher.finish()))
    }
    _ => Box::new(rand::thread_rng()),
  };

  if min == max {
    return min;
  }

  let (low, high) = if min < max { (min, max) } else { (max, min) };

  rng.gen_range(low..=high)
}

#[op2]
#[string]
fn op_vfile_set(#[string] full_path: String, #[string] content: String) -> String {
  add_virtual_file(full_path.as_str(), content.as_str());
  "".to_string()
}

#[op2]
#[string]
fn op_vfile_get(#[string] full_path: String) -> String {
  if let Some(v) = VIRTUAL_FILES
    .lock()
    .unwrap()
    .iter()
    .find(|(p, _)| *p == full_path)
  {
    return v.1.clone();
  }
  "".to_string()
}

#[op2]
#[string]
fn op_gen_uid(length: i32, #[string] seed: Option<String>) -> String {
  if let Some(seed_str) = seed {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed_str.hash(&mut hasher);

    let seed = hasher.finish();
    let mut rng = StdRng::seed_from_u64(seed);

    (0..length)
      .map(|_| rng.sample(Alphanumeric) as char)
      .collect()
  } else {
    let mut rng = rand::thread_rng();

    (0..length)
      .map(|_| rng.sample(Alphanumeric) as char)
      .collect()
  }
}

