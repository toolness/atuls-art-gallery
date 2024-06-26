extends MultiplayerSynchronizer

class_name PlayerInput

@export var input_direction: Vector2

@export var basis: Transform3D

func _ready():
    pass

func is_authority() -> bool:
    return get_multiplayer_authority() == multiplayer.get_unique_id()

func _process(_delta: float) -> void:
    if is_authority():
        input_direction = Input.get_vector("move_left", "move_right", "move_forward", "move_back")
