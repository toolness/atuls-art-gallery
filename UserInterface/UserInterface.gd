extends CanvasLayer
class_name UI

@onready var pause_screen: Control = $PauseScreen
@onready var pause_container: CenterContainer = %PauseContainer
@onready var main_menu_container: CenterContainer = %MainMenuContainer
@onready var settings_container: SettingsContainer = %SettingsContainer
@onready var join_game_container: CenterContainer = %JoinGameContainer
@onready var host_field: LineEdit = %HostField
@onready var resume_button: Button = %ResumeButton
@onready var start_button: Button = %StartButton
@onready var color_rect_fader: ColorRect = $ColorRectFader
@onready var inspect_mode_hints: Control = %InspectModeHints
@onready var hints: Control = %Hints

@onready var reticle: Reticle = %Reticle

@onready var error_dialog: AcceptDialog = %ErrorDialog

@export var start_level: PackedScene

var in_main_menu: bool = false

signal before_reload(hard_reset: bool)

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
	pause_container.visible = true
	main_menu_container.visible = false
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
	elif event.is_action_pressed("toggle_fullscreen"):
		settings_container.toggle_fullscreen()

	if in_main_menu:
		return

	if event.is_action_pressed("cycle_debug_draw"):
		cycle_debug_draw()
	elif event.is_action_pressed("reset_and_reload_scene"):
		reload_current_scene(true)
	elif event.is_action_pressed("reload_scene"):
		reload_current_scene(false)
	elif event.is_action_pressed("toggle_reticle"):
		toggle_reticle()

func start_game() -> void:
	get_tree().change_scene_to_packed(start_level)
	hints.visible = true
	main_menu_container.visible = false
	pause_container.visible = true
	in_main_menu = false

func _on_start_button_pressed() -> void:
	var tween := create_tween()
	fade_out(tween)
	tween.tween_callback(start_game)
	fade_in(tween)
	paused = false

func _on_resume_button_pressed() -> void:
	paused = false

func _on_settings_button_pressed() -> void:
	main_menu_container.visible = false
	pause_container.visible = false
	settings_container.visible = true
	settings_container.focus()

func _on_quit_button_pressed() -> void:
	get_tree().root.propagate_notification(NOTIFICATION_WM_CLOSE_REQUEST)

func _notification(what):
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		get_tree().quit()

func _on_settings_container_exit() -> void:
	settings_container.visible = false
	if in_main_menu:
		main_menu_container.visible = true
		start_button.grab_focus()
	else:
		pause_container.visible = true
		resume_button.grab_focus()

func toggle_reticle() -> void:
	reticle.visible = not reticle.visible
	inspect_mode_hints.visible = reticle.visible

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

func get_main_player() -> Player:
	for maybe_player in get_tree().get_nodes_in_group("Player"):
		if maybe_player is Player:
			var player: Player = maybe_player
			if player.is_main_player:
				return player
	print("Warning: main player not found!")
	return null

# Fade the screen out, reload the level and fade back in.
func reload_current_scene(hard_reset: bool) -> void:
	before_reload.emit(hard_reset)
	# Store a reference to the player to pass its settings onto the next player.
	var player = get_main_player()
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
		var new_player = get_main_player()
		# Apply cached settings, but don't update position.
		new_player.view = view
		new_player.zoom = zoom
		)
	fade_in(tween)

func show_fatal_error(message: String):
	Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
	error_dialog.dialog_text = "Alas, a fatal error occurred:\n\n" + message
	error_dialog.popup_centered()

func show_main_menu():
	in_main_menu = true
	paused = true
	pause_container.visible = false
	main_menu_container.visible = true
	join_game_container.visible = false
	start_button.grab_focus()

func _on_join_button_pressed():
	main_menu_container.visible = false
	join_game_container.visible = true

func _on_connect_button_pressed():
	Lobby.IS_CLIENT = true
	Lobby.HOST = host_field.text
