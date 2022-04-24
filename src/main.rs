#![windows_subsystem = "windows"]

extern crate nfd;
extern crate csv;

mod file_reader;
mod bin_compare;

use nuklear::*;
extern crate nuklear_backend_gfx;

extern crate image;

extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

use nuklear_backend_gfx::{Drawer, GfxBackend};

pub type ColorFormat = gfx::format::Rgba8;
pub type DepthFormat = gfx::format::DepthStencil;

const MAX_VERTEX_MEMORY: usize = 512 * 1024;
const MAX_ELEMENT_MEMORY: usize = 128 * 1024;
const MAX_COMMANDS_MEMORY: usize = 64 * 1024;

use nfd::Response;
use csv::Writer;

use std::error::Error;
use glutin::dpi::{LogicalSize};

struct BasicState {
    window_pos: nuklear::Vec2,
    window_size: nuklear::Vec2,
    running: bool,
    first_file_path: std::string::String,
    second_file_path: std::string::String
}

struct Bin {
    path: std::string::String, 
    loaded: bool,  
    data: Vec<u8>
}

struct Bins { 
    first_bin: Bin, 
    second_bin: Bin,
    compared: bool
}

struct Media {
    font_atlas: FontAtlas,
    font_14: FontID,
    font_20: FontID,
}

fn main() {
    let gl_version = glutin::GlRequest::GlThenGles {
        opengles_version: (2, 0),
        opengl_version: (3, 3),
    };

    let builder = glutin::WindowBuilder::new().with_title("Bin Diff").with_dimensions(glutin::dpi::LogicalSize { width: 800., height: 600. });

    let context = glutin::ContextBuilder::new().with_gl(gl_version).with_vsync(true).with_srgb(false).with_depth_buffer(24);

    let mut event_loop = glutin::EventsLoop::new();
    let (window, mut device, mut factory, main_color, mut main_depth) = gfx_window_glutin::init::<ColorFormat, DepthFormat>(builder, context, &event_loop).unwrap();
    let mut encoder: gfx::Encoder<_, _> = factory.create_command_buffer().into();

    let mut cfg = FontConfig::with_size(0.0);
    cfg.set_oversample_h(3);
    cfg.set_oversample_v(2);
    cfg.set_glyph_range(font_cyrillic_glyph_ranges());
    cfg.set_ttf(include_bytes!("../res/fonts/Roboto-Regular.ttf"));

    let mut allo = Allocator::new_vec();

    let mut drawer = Drawer::new(&mut factory, main_color, 36, MAX_VERTEX_MEMORY, MAX_ELEMENT_MEMORY, Buffer::with_size(&mut allo, MAX_COMMANDS_MEMORY), GfxBackend::OpenGlsl150);

    let mut atlas = FontAtlas::new(&mut allo);

    cfg.set_ttf_data_owned_by_atlas(false);
    cfg.set_size(14f32);
    let font_14 = atlas.add_font_with_config(&cfg).unwrap();

    let font_tex = {
        let (b, w, h) = atlas.bake(FontAtlasFormat::Rgba32);
        drawer.add_texture(&mut factory, b, w, h)
    };

    let mut null = DrawNullTexture::default();

    atlas.end(font_tex, Some(&mut null));
    atlas.cleanup();

    let mut ctx = Context::new(&mut allo, atlas.font(font_14).unwrap().handle());

    let mut basic_state = BasicState {
        window_pos: Vec2::default(), 
        window_size: Vec2 { 
            x: 800f32, 
            y: 600f32
        }, 
        running: true,
        first_file_path: std::string::String::from(""),
        second_file_path: std::string::String::from(""),
    };

    let mut media = Media {
        font_atlas: atlas,
        font_14,
        font_20: font_14
    };

    let first_bin = Bin { 
        path: std::string::String::from(""), 
        loaded: false, 
        data: Vec::default()
    };

    let second_bin = Bin { 
        path: std::string::String::from(""), 
        loaded: false, 
        data: Vec::default()
    };

    let mut bins = Bins { 
        first_bin,
        second_bin,
        compared: false
    };

    let mut offsets = Vec::new();

    let mut mx = 0;
    let mut my = 0;

    let mut config = ConvertConfig::default();
    config.set_null(null.clone());
    config.set_circle_segment_count(22);
    config.set_curve_segment_count(22);
    config.set_arc_segment_count(22);
    config.set_global_alpha(1.0f32);
    config.set_shape_aa(AntiAliasing::On);
    config.set_line_aa(AntiAliasing::On);

    while basic_state.running {
        ctx.input_begin();

        event_loop.poll_events(|event| {
            if let glutin::Event::WindowEvent { event, .. } = event {
                match event {
                    glutin::WindowEvent::CloseRequested => basic_state.running = false,
                    glutin::WindowEvent::ReceivedCharacter(c) => {
                        ctx.input_unicode(c);
                    }
                    glutin::WindowEvent::KeyboardInput {
                        input: glutin::KeyboardInput { state, virtual_keycode, .. },
                        ..
                    } => {
                        if let Some(k) = virtual_keycode {
                            let key = match k {
                                glutin::VirtualKeyCode::Back => Key::Backspace,
                                glutin::VirtualKeyCode::Delete => Key::Del,
                                glutin::VirtualKeyCode::Up => Key::Up,
                                glutin::VirtualKeyCode::Down => Key::Down,
                                glutin::VirtualKeyCode::Left => Key::Left,
                                glutin::VirtualKeyCode::Right => Key::Right,
                                _ => Key::None,
                            };

                            ctx.input_key(key, state == glutin::ElementState::Pressed);
                        }
                    }
                    glutin::WindowEvent::CursorMoved { position: glutin::dpi::LogicalPosition { x, y }, .. } => {
                        mx = x as i32;
                        my = y as i32;
                        ctx.input_motion(x as i32, y as i32);
                    }
                    glutin::WindowEvent::MouseInput { state, button, .. } => {
                        let button = match button {
                            glutin::MouseButton::Left => Button::Left,
                            glutin::MouseButton::Middle => Button::Middle,
                            glutin::MouseButton::Right => Button::Right,
                            _ => Button::Max,
                        };

                        ctx.input_button(button, mx, my, state == glutin::ElementState::Pressed)
                    }
                    glutin::WindowEvent::MouseWheel { delta, .. } => {
                        if let glutin::MouseScrollDelta::LineDelta(x, y) = delta {
                            ctx.input_scroll(Vec2 { x: x * 22f32, y: y * 22f32 });
                        }
                    }
                    glutin::WindowEvent::Resized(logical_size) => {
                        let mut main_color = drawer.col.clone().unwrap();
                        gfx_window_glutin::update_views(&window, &mut main_color, &mut main_depth);
                        drawer.col = Some(main_color);
                        basic_state.window_size.x = logical_size.width as f32;
                        basic_state.window_size.y = logical_size.height as f32;
                    }
                    glutin::WindowEvent::Moved(position) => {
                        basic_state.window_pos.x = position.x as f32;
                        basic_state.window_pos.y = position.y as f32;
                    }
                    _ => (),
                }
            }
        });
        ctx.input_end();

        if !basic_state.running {
            break;
        }

        // println!("{:?}", event);
        let LogicalSize { width, height } = window.get_inner_size().unwrap();
        let scale = Vec2 { x: 1., y: 1. };

        draw_left_menu_bar(&mut ctx, &mut media, &mut basic_state, &mut bins);

        if !bins.first_bin.path.is_empty() && !bins.first_bin.loaded {
            bins.first_bin.data = file_reader::read_bin(&bins.first_bin.path);
            bins.first_bin.loaded = true;
        }

        if !bins.second_bin.path.is_empty() && !bins.second_bin.loaded {
            bins.second_bin.data = file_reader::read_bin(&bins.second_bin.path);
            bins.second_bin.loaded = true;
        }

        if bins.first_bin.loaded && bins.second_bin.loaded {
            if !bins.compared {
                match bin_compare::compare(4,&bins.first_bin.data, &bins.second_bin.data) {
                    Ok(off) => {
                        offsets = off.clone();
                        for offset in off {
                            dbg!("{:#04x}{:#04x}", offset.0, offset.1);
                        }

                        // for b in off {
                        //     println!("{}", b);
                        // }
                    }
                    Err(msg) => {
                        //TODO(Demi): Handle error
                        println!("{}", msg);
                    }
                }
                bins.compared = true;
            }

            draw_comparison_window(&mut ctx, &mut media, &mut basic_state, &offsets)
        }

        encoder.clear(drawer.col.as_ref().unwrap(), [0.1f32, 0.2f32, 0.3f32, 1.0f32]);
        drawer.draw(&mut ctx, &mut config, &mut encoder, &mut factory, width as u32, height as u32, scale);
        encoder.flush(&mut device);
        window.swap_buffers().unwrap();

        ::std::thread::sleep(::std::time::Duration::from_millis(20));

        ctx.clear();
    }
}

fn ui_header(ctx: &mut Context, media: &mut Media, title: &str) {
    ctx.style_set_font(media.font_atlas.font(media.font_14).unwrap().handle());
    ctx.layout_row_dynamic(20f32, 1);
    ctx.text(title, TextAlignment::Left as Flags);
}

const RATIO_W: [f32; 2] = [0.15f32, 0.85f32];
fn ui_widget(ctx: &mut Context, media: &mut Media, height: f32) {
    ctx.style_set_font(media.font_atlas.font(media.font_14).unwrap().handle());
    ctx.layout_row(LayoutFormat::Dynamic, height, &RATIO_W);
    // ctx.layout_row_dynamic(height, 1);
    ctx.spacing(1);
}

fn write_to_csv(path: std::string::String, offsets: &Vec<(usize, u128)>) -> Result<(), Box<dyn Error>> {
    let mut writer = Writer::from_path(path)?;

    writer.write_record(&["Offset", "Original Position"])?;

    for offset in offsets {
        writer.write_record(&[
            std::string::String::from(format!("{:#08x}", offset.1)),
            std::string::String::from(format!("{:#08x}", offset.0))
        ])?;
    }

    writer.flush()?;

    Ok(())
}

fn draw_comparison_window(ctx: &mut Context, media: &mut Media, state: &mut BasicState, offsets: &Vec<(usize, u128)>) {
    // ctx.style_set_font(dr.font_by_id(media.font_20).unwrap());
    ctx.style_set_font(media.font_atlas.font(media.font_20).unwrap().handle());

    ctx.begin(
        nk_string!("Diff View"),
        Rect { x: state.window_size.x / 3f32, y: 0f32, w: (state.window_size.x / 3f32) * 2f32, h: state.window_size.y },
        0 as Flags        
    );

    let mut label_string = std::string::String::from("");

    label_string.push_str(&std::string::String::from(format!("{} offsets found", offsets.len())));

    ui_widget(ctx, media, 35f32);
    ctx.label(String::from(label_string), TextAlignment::Centered as Flags); 
    
    ui_widget(ctx, media, 35f32);
    if ctx.button_text("Save Offsets") {
        dbg!("Save dialog button pressed");

        match nfd::open_save_dialog(std::option::Option::from("csv"), None).unwrap() {
            Response::Okay(path) => match write_to_csv(path, offsets) {
                Ok(_) => {
                    println!("Wrote the csv file.")
                }
                Err(_) => {
                    println!("Could not write the csv file")
                }
            },
            Response::OkayMultiple(_) => {}
            Response::Cancel => {}
        }

    }

    ctx.style_set_font(media.font_atlas.font(media.font_14).unwrap().handle());
    ctx.end();
}

fn draw_left_menu_bar(ctx: &mut Context, media: &mut Media, state: &mut BasicState, bins: &mut Bins) {
    ctx.style_set_font(media.font_atlas.font(media.font_20).unwrap().handle());
    ctx.begin(
        nk_string!("Tools"),
        Rect { x: 0f32, y: 0f32, w: state.window_size.x / 3f32, h: state.window_size.y },
        0 as Flags        
    );

    // ------------------------------------------------
    //                  BUTTON
    // ------------------------------------------------
    ui_header(ctx,  media, "Load Bins");
    ui_widget(ctx,  media, 35f32);

    if ctx.button_text("First Bin") {
        dbg!("First file dialog pressed");

        let result = nfd::open_file_dialog(std::option::Option::from("bin"), None).unwrap();

        match result { 
            Response::Okay(file_path) => {
                bins.first_bin.path = file_path.clone();
                state.first_file_path = file_path;
            },
            Response::OkayMultiple(_files) => (), 
            Response::Cancel => ()
        }
    }
    ui_widget(ctx,  media, 35f32);
    if ctx.button_text("Second Bin") {
        dbg!("Second file dialog pressed");

        let result = nfd::open_file_dialog(std::option::Option::from("bin"), None).unwrap();

        match result { 
            Response::Okay(file_path) => {
                bins.second_bin.path = file_path.clone();
                state.second_file_path = file_path;
            },
            Response::OkayMultiple(_files) => (), 
            Response::Cancel => ()
        }
    }
    
    ui_widget(ctx,  media, 35f32);
    ctx.label(String::from(format!("First File: {}", state.first_file_path)), TextAlignment::Left as Flags);

    ui_widget(ctx,  media, 35f32);
    ctx.label(String::from(format!("Second File: {}", state.second_file_path)), TextAlignment::Left as Flags);

    // ui_widget(ctx, dr, media, 35f32); 
    // if ctx.button_text("Save Comparison") { 
    //     dbg!("Save the compare"
    // }

    ctx.style_set_font(media.font_atlas.font(media.font_14).unwrap().handle());
    ctx.end();
}
