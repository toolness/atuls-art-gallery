extends MultiplayerSynchronizer

class_name PlayerInput

@export var input_direction: Vector2

var clicked := false

var jumped := false

var teleported := false

var teleported_via_teleport_dialog := false

var teleported_via_teleport_dialog_id := 0

func _ready():
	if is_authority():
			UserInterface.teleport_dialog.teleport_requested.connect(_on_teleport_requested)

func _on_teleport_requested(gallery_id: int):
	UserInterface.hide_teleport_dialog()

	teleport_via_teleport_dialog.rpc(gallery_id)

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

@rpc("call_local")
func teleport_via_teleport_dialog(gallery_id: int):
	teleported_via_teleport_dialog = true
	teleported_via_teleport_dialog_id = gallery_id

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
		# The fact that we have to test against teleport_dialog is extremely stupid but
		# right now teleport_dialog is ctrl+T and teleport is T and *both* are triggered
		# when the user presses the latter.
		if event.is_action_pressed("teleport") and not event.is_action_pressed("teleport_dialog"):
			teleport.rpc()
