extends MultiplayerSynchronizer

class_name PlayerInput

@export var input_direction: Vector2

var clicked := false

func _ready():
    pass

func is_authority() -> bool:
    return get_multiplayer_authority() == multiplayer.get_unique_id()

func _process(_delta: float) -> void:
    if is_authority():
        input_direction = Input.get_vector("move_left", "move_right", "move_forward", "move_back")

@rpc("call_local")
func click():
    clicked = true

func _unhandled_input(event: InputEvent) -> void:
    if is_authority():
        # Capture the mouse if it is uncaptured.
        if event.is_action_pressed("click"):
            if Input.get_mouse_mode() != Input.MOUSE_MODE_CAPTURED:
                Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)
            else:
                click.rpc()
