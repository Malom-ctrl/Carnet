use crate::Terminal;
use std::ffi::{CString, c_char, c_void};
use std::io;
use std::ptr;

// --- GLib / GIO FFI definitions ---
#[repr(C)]
pub struct GError {
    pub domain: u32,
    pub code: i32,
    pub message: *mut c_char,
}
#[repr(C)]
pub struct GAsyncResult {
    _private: [u8; 0],
}
#[repr(C)]
pub struct GBytes {
    _private: [u8; 0],
}
#[repr(C)]
pub struct GMainLoop {
    _private: [u8; 0],
}

type GAsyncReadyCallback =
    extern "C" fn(source: *mut c_void, res: *mut GAsyncResult, user_data: *mut c_void);

#[link(name = "glib-2.0")]
unsafe extern "C" {
    fn g_main_loop_new(context: *mut c_void, is_running: i32) -> *mut GMainLoop;
    fn g_main_loop_run(loop_: *mut GMainLoop);
    fn g_main_loop_quit(loop_: *mut GMainLoop);
    fn g_main_loop_unref(loop_: *mut GMainLoop);
    fn g_bytes_new(data: *const c_void, size: usize) -> *mut GBytes;
    fn g_bytes_unref(bytes: *mut GBytes);
    fn g_free(ptr: *mut c_void);
}

#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_object_unref(object: *mut c_void);
}

// --- Glycin 2.x FFI definitions ---
#[repr(C)]
pub struct GlycinLoader {
    _private: [u8; 0],
}
#[repr(C)]
pub struct GlycinImage {
    _private: [u8; 0],
}

#[link(name = "glycin-2")]
unsafe extern "C" {
    fn gly_loader_new_for_bytes(bytes: *mut GBytes, mime_type: *const c_char) -> *mut GlycinLoader;
    fn gly_loader_load_async(
        loader: *mut GlycinLoader,
        cancellable: *mut c_void,
        callback: GAsyncReadyCallback,
        user_data: *mut c_void,
    );
    fn gly_loader_load_finish(
        loader: *mut GlycinLoader,
        res: *mut GAsyncResult,
        error: *mut *mut GError,
    ) -> *mut GlycinImage;
    fn gly_image_get_width(image: *mut GlycinImage) -> u32;
    fn gly_image_get_height(image: *mut GlycinImage) -> u32;
}

pub struct ImageProcessor;

struct LoadContext {
    loop_ptr: *mut GMainLoop,
    image_ptr: *mut GlycinImage,
}

fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = match chunk.len() {
            3 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8 | (chunk[2] as u32),
            2 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8,
            1 => (chunk[0] as u32) << 16,
            _ => unreachable!(),
        };
        result.push(CHARSET[((b >> 18) & 0x3F) as usize] as char);
        result.push(CHARSET[((b >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARSET[((b >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push((CHARSET[(b & 0x3F) as usize]) as char);
        } else {
            result.push('=');
        }
    }
    result
}

impl ImageProcessor {
    pub fn get_image_info(data: &[u8], mime_type: &str) -> Option<(u32, u32)> {
        unsafe {
            let bytes = g_bytes_new(data.as_ptr() as *const c_void, data.len());
            let mime_c = CString::new(mime_type).ok()?;
            let loader = gly_loader_new_for_bytes(bytes, mime_c.as_ptr());
            g_bytes_unref(bytes);

            if loader.is_null() {
                return None;
            }

            let main_loop = g_main_loop_new(ptr::null_mut(), 0);
            let mut context = LoadContext {
                loop_ptr: main_loop,
                image_ptr: ptr::null_mut(),
            };

            extern "C" fn on_load_done(
                loader: *mut c_void,
                res: *mut GAsyncResult,
                user_data: *mut c_void,
            ) {
                unsafe {
                    let ctx = &mut *(user_data as *mut LoadContext);
                    let mut error: *mut GError = ptr::null_mut();
                    ctx.image_ptr =
                        gly_loader_load_finish(loader as *mut GlycinLoader, res, &mut error);

                    if !error.is_null() {
                        g_free(error as *mut c_void);
                    }

                    g_main_loop_quit(ctx.loop_ptr);
                }
            }

            gly_loader_load_async(
                loader,
                ptr::null_mut(),
                on_load_done,
                &mut context as *mut _ as *mut c_void,
            );

            g_main_loop_run(main_loop);

            let result = if !context.image_ptr.is_null() {
                let w = gly_image_get_width(context.image_ptr);
                let h = gly_image_get_height(context.image_ptr);
                Some((w, h))
            } else {
                None
            };

            // Cleanup
            if !context.image_ptr.is_null() {
                g_object_unref(context.image_ptr as *mut c_void);
            }
            g_object_unref(loader as *mut c_void);
            g_main_loop_unref(main_loop);

            result
        }
    }

    pub fn render_image(
        terminal: &mut Terminal,
        data: &[u8],
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    ) -> io::Result<()> {
        let term_var = std::env::var("TERM").unwrap_or_default();
        let in_tmux = std::env::var("TMUX").is_ok() || term_var.contains("tmux");
        let f = if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            100
        } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            68
        } else {
            100
        };
        let info = Self::get_image_info(data, if f == 100 { "image/png" } else { "image/jpeg" });
        let b64 = base64_encode(data);
        let chunks: Vec<&[u8]> = b64.as_bytes().chunks(4096).collect();
        let mut c_r = None;
        if let Some((img_w, img_h)) = info {
            let cell_ratio = 2.0;
            let scale = (w as f32 / img_w as f32).min((h as f32 * cell_ratio) / img_h as f32);
            let c = (img_w as f32 * scale).max(1.0) as u16;
            let r = ((img_h as f32 * scale) / cell_ratio).max(1.0) as u16;
            c_r = Some((c, r));
            terminal
                .move_to(y + (h.saturating_sub(r) / 2), x + (w.saturating_sub(c) / 2))
                .ok();
        }
        for (i, chunk) in chunks.iter().enumerate() {
            let m = if i < chunks.len() - 1 { 1 } else { 0 };
            let chunk_str = std::str::from_utf8(chunk).unwrap_or("");
            let mut seq = if i == 0 {
                let mut keys = format!("a=T,f={},t=d,m={}", f, m);
                if let Some((c, r)) = c_r {
                    keys.push_str(&format!(",c={},r={}", c, r));
                }
                format!("\x1b_G{};{}\x1b\\", keys, chunk_str)
            } else {
                format!("\x1b_Gm={};{}\x1b\\", m, chunk_str)
            };
            if in_tmux {
                seq = format!("\x1bPtmux;{}\x1b\\", seq.replace("\x1b", "\x1b\x1b"));
            }
            terminal.print(&seq).ok();
        }
        Ok(())
    }
}
