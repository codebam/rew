use deno_core::{extension, op2, OpState, CoreError};
use std::cell::RefCell;
use std::rc::Rc;

extension!(
  rew_os,
  ops = [
    op_os_info_os,
    op_os_info_arch,
    op_os_info_family,
    op_terminal_size,
  ]
);

#[op2]
#[string]
fn op_os_info_os(_: Rc<RefCell<OpState>>) -> Result<String, CoreError> {
  Ok(std::env::consts::OS.to_string())
}

#[op2]
#[string]
fn op_os_info_arch(_: Rc<RefCell<OpState>>) -> Result<String, CoreError> {
  Ok(std::env::consts::ARCH.to_string())
}

#[op2]
#[string]
fn op_os_info_family(_: Rc<RefCell<OpState>>) -> Result<String, CoreError> {
  Ok(std::env::consts::FAMILY.to_string())
}

#[op2]
#[serde]
fn op_terminal_size() -> Result<(u16, u16), std::io::Error> {
  #[cfg(unix)]
  {
    use libc::{STDOUT_FILENO, TIOCGWINSZ, ioctl, winsize};

    let mut ws: winsize = unsafe { std::mem::zeroed() };

    let result = unsafe { ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut ws) };

    if result == -1 {
      return Err(std::io::Error::last_os_error());
    }

    Ok((ws.ws_col, ws.ws_row))
  }

  #[cfg(windows)]
  {
    use std::mem::zeroed;
    use std::ptr::null_mut;
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::STD_OUTPUT_HANDLE;
    use winapi::um::wincon::{CONSOLE_SCREEN_BUFFER_INFO, GetConsoleScreenBufferInfo};

    unsafe {
      let handle = GetStdHandle(STD_OUTPUT_HANDLE);
      if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
      }

      let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = zeroed();
      if GetConsoleScreenBufferInfo(handle, &mut csbi) == 0 {
        return Err(std::io::Error::last_os_error());
      }

      let width = (csbi.srWindow.Right - csbi.srWindow.Left + 1) as u16;
      let height = (csbi.srWindow.Bottom - csbi.srWindow.Top + 1) as u16;

      Ok((width, height))
    }
  }
}

