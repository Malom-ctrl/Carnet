use crate::Terminal;
use std::ffi::{c_char, c_void};
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
#[repr(C)]
pub struct GlycinFrame {
    _private: [u8; 0],
}

#[link(name = "glycin-2")]
unsafe extern "C" {
    fn gly_loader_new_for_bytes(bytes: *mut GBytes) -> *mut GlycinLoader;
    fn gly_loader_set_accepted_memory_formats(loader: *mut GlycinLoader, format: u32);
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
    fn gly_image_get_specific_frame_async(
        image: *mut GlycinImage,
        frame_request: *mut c_void,
        cancellable: *mut c_void,
        callback: GAsyncReadyCallback,
        user_data: *mut c_void,
    );
    fn gly_image_get_specific_frame_finish(
        image: *mut GlycinImage,
        res: *mut GAsyncResult,
        error: *mut *mut GError,
    ) -> *mut GlycinFrame;
    fn gly_frame_request_new() -> *mut c_void;
    fn gly_frame_get_buf_bytes(frame: *mut GlycinFrame) -> *mut GBytes;
    fn g_bytes_get_data(bytes: *mut GBytes, size: *mut usize) -> *const c_void;
}

pub struct ImageProcessor;

struct LoadContext {
    loop_ptr: *mut GMainLoop,
    image_ptr: *mut GlycinImage,
    frame_ptr: *mut GlycinFrame,
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
    pub fn get_image_info(data: &[u8], _mime_type: &str) -> Option<(u32, u32)> {
        let loader = unsafe {
            let bytes = g_bytes_new(data.as_ptr() as *const c_void, data.len());
            let loader = gly_loader_new_for_bytes(bytes);
            g_bytes_unref(bytes);
            loader
        };

        if loader.is_null() {
            return None;
        }

        let main_loop = unsafe { g_main_loop_new(ptr::null_mut(), 0) };
        let mut context = LoadContext {
            loop_ptr: main_loop,
            image_ptr: ptr::null_mut(),
            frame_ptr: ptr::null_mut(),
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

        unsafe {
            gly_loader_load_async(
                loader,
                ptr::null_mut(),
                on_load_done,
                &mut context as *mut _ as *mut c_void,
            );

            g_main_loop_run(main_loop);
        }

        let result = if !context.image_ptr.is_null() {
            let w = unsafe { gly_image_get_width(context.image_ptr) };
            let h = unsafe { gly_image_get_height(context.image_ptr) };
            Some((w, h))
        } else {
            None
        };

        // Cleanup
        unsafe {
            if !context.image_ptr.is_null() {
                g_object_unref(context.image_ptr as *mut c_void);
            }
            g_object_unref(loader as *mut c_void);
            g_main_loop_unref(main_loop);
        }

        result
    }

    pub fn decode_image_to_rgba(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
        let loader = unsafe {
            let bytes = g_bytes_new(data.as_ptr() as *const c_void, data.len());
            let loader = gly_loader_new_for_bytes(bytes);
            g_bytes_unref(bytes);
            loader
        };

        if loader.is_null() {
            return None;
        }

        // Request RGBA (8-bit)
        unsafe {
            gly_loader_set_accepted_memory_formats(loader, 1 << 5);
        }

        let main_loop = unsafe { g_main_loop_new(ptr::null_mut(), 0) };
        let mut context = LoadContext {
            loop_ptr: main_loop,
            image_ptr: ptr::null_mut(),
            frame_ptr: ptr::null_mut(),
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

                if !ctx.image_ptr.is_null() {
                    extern "C" fn on_frame_done(
                        image: *mut c_void,
                        res: *mut GAsyncResult,
                        user_data: *mut c_void,
                    ) {
                        unsafe {
                            let ctx = &mut *(user_data as *mut LoadContext);
                            let mut error: *mut GError = ptr::null_mut();
                            ctx.frame_ptr = gly_image_get_specific_frame_finish(
                                image as *mut GlycinImage,
                                res,
                                &mut error,
                            );
                            if !error.is_null() {
                                g_free(error as *mut c_void);
                            }
                            g_main_loop_quit(ctx.loop_ptr);
                        }
                    }
                    let request = gly_frame_request_new();
                    gly_image_get_specific_frame_async(
                        ctx.image_ptr,
                        request,
                        ptr::null_mut(),
                        on_frame_done,
                        user_data,
                    );
                    if !request.is_null() {
                        g_object_unref(request);
                    }
                } else {
                    g_main_loop_quit(ctx.loop_ptr);
                }
            }
        }

        unsafe {
            gly_loader_load_async(
                loader,
                ptr::null_mut(),
                on_load_done,
                &mut context as *mut _ as *mut c_void,
            );

            g_main_loop_run(main_loop);
        }

        let result = if !context.frame_ptr.is_null() {
            unsafe {
                let w = gly_image_get_width(context.image_ptr);
                let h = gly_image_get_height(context.image_ptr);
                let g_bytes = gly_frame_get_buf_bytes(context.frame_ptr);
                let mut size: usize = 0;
                let data_ptr = g_bytes_get_data(g_bytes, &mut size);
                let mut rgba = Vec::with_capacity(size);
                std::ptr::copy_nonoverlapping(data_ptr as *const u8, rgba.as_mut_ptr(), size);
                rgba.set_len(size);
                Some((w, h, rgba))
            }
        } else {
            None
        };

        // Cleanup
        unsafe {
            if !context.frame_ptr.is_null() {
                g_object_unref(context.frame_ptr as *mut c_void);
            }
            if !context.image_ptr.is_null() {
                g_object_unref(context.image_ptr as *mut c_void);
            }
            g_object_unref(loader as *mut c_void);
            g_main_loop_unref(main_loop);
        }

        result
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

        let decoded = Self::decode_image_to_rgba(data);
        if decoded.is_none() {
            return Ok(());
        }
        let (img_w, img_h, rgba) = decoded.unwrap();

        let b64 = base64_encode(&rgba);
        let chunks: Vec<&[u8]> = b64.as_bytes().chunks(4096).collect();

        let cell_ratio = 2.0;
        let scale = (w as f32 / img_w as f32).min((h as f32 * cell_ratio) / img_h as f32);
        let c = (img_w as f32 * scale).max(1.0) as u16;
        let r = ((img_h as f32 * scale) / cell_ratio).max(1.0) as u16;

        let mut image_full_seq = String::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let m = if i < chunks.len() - 1 { 1 } else { 0 };
            let chunk_str = std::str::from_utf8(chunk).unwrap_or("");
            let mut seq = if i == 0 {
                let mut keys = format!("a=T,f=32,s={},v={},t=d,m={}", img_w, img_h, m);
                keys.push_str(&format!(",c={},r={}", c, r));
                format!("\x1b_G{};{}\x1b\\", keys, chunk_str)
            } else {
                format!("\x1b_Gm={};{}\x1b\\", m, chunk_str)
            };
            if in_tmux {
                seq = format!("\x1bPtmux;{}\x1b\\", seq.replace("\x1b", "\x1b\x1b"));
            }
            image_full_seq.push_str(&seq);
        }

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        rgba.hash(&mut hasher);
        let hash = hasher.finish();

        let target_x = x + (w.saturating_sub(c) / 2);
        let target_y = y + (h.saturating_sub(r) / 2);

        terminal.set_image(target_x, target_y, c, r, hash, image_full_seq);
        Ok(())
    }
}
