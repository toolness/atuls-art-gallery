use godot::prelude::*;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

#[derive(GodotClass)]
#[class(init)]
struct Boop {}

#[godot_api]
impl Boop {
    #[func]
    fn add(a: i32, b: i32) -> i32 {
        godot_print!("ADD {a} + {b}!");
        a + b
    }
}
