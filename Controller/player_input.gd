extends MultiplayerSynchronizer

class_name PlayerInput

@export var input_direction: Vector2

var clicked := false

var jumped := false

var teleported := false

func _ready():
	pass

func is_authority() -> bool:
	return get_multiplayer_authority() == multiplayer.get_unique_id()

func _process(_delta: float) -> void:
	if is_authority():
		if get_viewport().gui_get_focus_owner():
			# Some control has keyboard focus. If the player is inputting via the keyboard, we don't
			# want their input to _both_ move them around _and_ type stuff into the GUI, so just
			# return early.
			return
		input_direction = Input.get_vector("move_left", "move_right", "move_forward", "move_back")

@rpc("call_local")
func click():
	clicked = true

@rpc("call_local")
func jump():
	jumped = true

@rpc("call_local")
func teleport():
	teleported = true

func _unhandled_input(event: InputEvent) -> void:
	if is_authority():
		if event.is_action_pressed("click"):
			# Capture the mouse if it is uncaptured.
			if Input.get_mouse_mode() != Input.MOUSE_MODE_CAPTURED:
				Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)
			elif UserInterface.reticle.visible:
				click.rpc()
		if event.is_action_pressed("jump"):
			jump.rpc()
		if event.is_action_pressed("teleport"):
			teleport.rpc()
