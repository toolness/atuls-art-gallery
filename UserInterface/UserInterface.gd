extends CanvasLayer
class_name UI

@onready var pause_screen: Control = $PauseScreen
@onready var pause_container: CenterContainer = %PauseContainer
@onready var settings_container: SettingsContainer = %SettingsContainer
@onready var resume_button: Button = %ResumeButton
@onready var color_rect_fader: ColorRect = $ColorRectFader

@onready var reticle: TextureRect = %Reticle

@onready var error_dialog: AcceptDialog = %ErrorDialog

var paused := false:
	set(value):
		paused = value
		pause_screen.visible = paused
		if paused:
			# Make the mouse visible, focus the resume button and pause the tree.
			Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
			resume_button.grab_focus()
			# This menu ignores pause mode so it can still be used.
			get_tree().paused = true
		else:
			# Capture the mouse and unpause the game.
			Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)
			get_tree().paused = false
		
func _ready() -> void:
	fade_in(create_tween())
	paused = false
	pause_container.visible = true
	settings_container.visible = false

var DEBUG_DRAW_CYCLE: Array[Viewport.DebugDraw] = [
	Viewport.DEBUG_DRAW_DISABLED,
	Viewport.DEBUG_DRAW_OVERDRAW,
	Viewport.DEBUG_DRAW_WIREFRAME,
]

func cycle_debug_draw():
	var vp := get_viewport()
	var curr_index := DEBUG_DRAW_CYCLE.find(vp.debug_draw)
	if curr_index == -1:
		# This should never happen, but maybe some other code changed it,
		# so just fall back to 0.
		curr_index = 0
	var next_index := (curr_index + 1) % DEBUG_DRAW_CYCLE.size()
	vp.debug_draw = DEBUG_DRAW_CYCLE[next_index]

func _unhandled_input(event: InputEvent) -> void:
	# When the player presses the escape key, pause the game.
	if event.is_action_pressed("ui_cancel"):
		paused = true
	elif event.is_action_pressed("cycle_debug_draw"):
		cycle_debug_draw()
	elif event.is_action_pressed("reset_and_reload_scene"):
		reload_current_scene(true)
	elif event.is_action_pressed("reload_scene"):
		reload_current_scene(false)
	elif event.is_action_pressed("toggle_reticle"):
		toggle_reticle()
	elif event.is_action_pressed("toggle_fullscreen"):
		settings_container.toggle_fullscreen()

func _on_resume_button_pressed() -> void:
	paused = false

func _on_settings_button_pressed() -> void:
	pause_container.visible = false
	settings_container.visible = true
	settings_container.focus()

func _on_quit_button_pressed() -> void:
	get_tree().root.propagate_notification(NOTIFICATION_WM_CLOSE_REQUEST)

func _notification(what):
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		get_tree().quit()

func _on_settings_container_exit() -> void:
	pause_container.visible = true
	settings_container.visible = false
	resume_button.grab_focus()

func toggle_reticle() -> void:
	reticle.visible = not reticle.visible

func hide_reticle(is_hidden:bool) -> void:
	# Hide the aiming reticle. Useful for the third person camera.
	reticle.visible = not is_hidden


# Take an existing tween and add steps to fade the screen in.
func fade_in(tween_in: Tween):
	tween_in.tween_property(color_rect_fader, "color:a", 0.0, 0.5)
	tween_in.tween_callback(func(): color_rect_fader.visible = false)

# Take an existing tween and add steps to fade the screen out.
func fade_out(tween_in: Tween):
	color_rect_fader.visible = true
	tween_in.tween_property(color_rect_fader, "color:a", 1.0, 0.25).from(0.0)
	
# Update the reference to the player variable in the settings container.
func update_player(player_in: Player) -> void:
	settings_container.update_player(player_in)
	
# Fade the screen out, change level and fade back in.
func change_scene(next_scene: String, player_transform: Transform3D) -> void:
	# Store a reference to the player to pass its settings onto the next player.
	var player = get_tree().get_first_node_in_group("Player")
	# Stop movement and cache settings.
	player.set_physics_process(false)
	var zoom = player.zoom
	var view = player.view
	var tween := create_tween()
	fade_out(tween)
	tween.tween_callback(func(): get_tree().change_scene_to_file(next_scene))
	# Wait at least one frame for the scene to update and ready.
	tween.tween_interval(0.1)
	tween.tween_callback(func():
		# Apply the cached variable to the new player.
		var new_player = get_tree().get_first_node_in_group("Player")
		# Set the player's position in the new level.
		new_player.global_transform = player_transform
		new_player.view = view
		new_player.zoom = zoom
		)
	fade_in(tween)

func _get_gallery() -> InfiniteGallery:
	var gallery := get_tree().get_first_node_in_group("InfiniteGallery")
	assert(gallery is InfiniteGallery)
	return gallery

# Fade the screen out, reload the level and fade back in.
func reload_current_scene(hard_reset: bool) -> void:
	if hard_reset:
		_get_gallery().delete_state()
	else:
		_get_gallery().save_state()
	# Store a reference to the player to pass its settings onto the next player.
	var player = get_tree().get_first_node_in_group("Player")
	# Stop movement and cache settings.
	player.set_physics_process(false)
	var zoom = player.zoom
	var view = player.view
	var tween := create_tween()
	fade_out(tween)
	tween.tween_callback(func(): get_tree().reload_current_scene())
	# Wait at least one frame for the scene to update and ready.
	tween.tween_interval(0.1)
	tween.tween_callback(func(): 
		var new_player = get_tree().get_first_node_in_group("Player")
		# Apply cached settings, but don't update position.
		new_player.view = view
		new_player.zoom = zoom
		)
	fade_in(tween)

func show_fatal_error(message: String):
	Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
	error_dialog.dialog_text = "Alas, a fatal error occurred:\n\n" + message
	error_dialog.popup_centered()
