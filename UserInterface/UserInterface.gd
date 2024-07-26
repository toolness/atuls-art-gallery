extends CanvasLayer
class_name UI

@onready var pause_screen: Control = $PauseScreen
@onready var pause_container: CenterContainer = %PauseContainer
@onready var main_menu_container: MainMenuContainer = %MainMenuContainer
@onready var settings_container: SettingsContainer = %SettingsContainer
@onready var join_game_container: JoinGameContainer = %JoinGameContainer
@onready var layout_config_container: LayoutConfigContainer = %LayoutConfigContainer
@onready var layout_config_button: Button = %LayoutConfigButton
@onready var connection_status_label: Label = %ConnectionStatusLabel
@onready var version_label: Label = %VersionLabel
@onready var resume_button: Button = %ResumeButton
@onready var color_rect_fader: ColorRect = $ColorRectFader
@onready var inspect_mode_hints: Control = %InspectModeHints
@onready var hints: Control = %Hints

@onready var reticle: Reticle = %Reticle

@onready var error_dialog: AcceptDialog = %ErrorDialog

@export var start_level: PackedScene

var DISABLE_INITIAL_MOUSE_CAPTURE := false

var in_main_menu: bool = false

signal before_reload(hard_reset: bool)

signal potato_mode_changed

signal global_illumination_changed

signal debug_draw_changed(value: Viewport.DebugDraw)

var paused := false:
	set(value):
		paused = value
		pause_screen.visible = paused
		if paused:
			# Make the mouse visible, focus the resume button and pause the tree.
			Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
			resume_button.grab_focus()

			# For now we're only supporting the layout config (new gallery) button in
			# offline mode, and when the server is running. We're not going to support
			# clients doing this yet because they don't have authority over paintings.
			layout_config_button.visible = not Lobby.IS_CLIENT

			# This menu ignores pause mode so it can still be used.
			get_tree().paused = true
		else:
			# Capture the mouse and unpause the game.
			Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)
			get_tree().paused = false

var main_player: Player = null:
	set(value):
		main_player = value
		settings_container.update_main_player(main_player)

var potato_mode := false:
	set(value):
		potato_mode = value
		potato_mode_changed.emit()

var global_illumination := true:
	set(value):
		global_illumination = value
		global_illumination_changed.emit()

func _ready() -> void:
	fade_in(create_tween())
	pause_screen.visible = false
	pause_container.visible = true
	main_menu_container.visible = false
	join_game_container.visible = false
	settings_container.visible = false
	layout_config_container.visible = false
	version_label.text = ProjectSettings.get_setting("application/config/version")

	# Hook up main menu events.
	main_menu_container.start_button.pressed.connect(fade_out_and_start_game)
	main_menu_container.join_button.pressed.connect(_on_main_menu_join_button_pressed)
	main_menu_container.host_button.pressed.connect(_on_main_menu_host_button_pressed)
	main_menu_container.settings_button.pressed.connect(_on_settings_button_pressed)
	main_menu_container.quit_button.pressed.connect(_on_quit_button_pressed)

	# Hook up join game menu events.
	join_game_container.connect_button.pressed.connect(_on_join_menu_connect_button_pressed)
	join_game_container.back_button.pressed.connect(show_main_menu)

	# Hook up layout config menu events.
	layout_config_container.exit.connect(_on_layout_config_container_exit)
	layout_config_container.new_layout_complete.connect(_on_new_layout_complete)


func set_connection_status_text(value: String):
	connection_status_label.text = value

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
	debug_draw_changed.emit(vp.debug_draw)

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
	main_menu_container.visible = false
	pause_container.visible = true
	in_main_menu = false

func fade_out_and_start_game() -> void:
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
		main_menu_container.focus()
	else:
		pause_container.visible = true
		resume_button.grab_focus()

func toggle_reticle() -> void:
	hide_reticle(reticle.visible)


func hide_reticle(is_hidden:bool) -> void:
	# Hide the aiming reticle. Useful for the third person camera.
	reticle.visible = not is_hidden
	inspect_mode_hints.visible = reticle.visible


# Take an existing tween and add steps to fade the screen in.
func fade_in(tween_in: Tween):
	tween_in.tween_property(color_rect_fader, "color:a", 0.0, 0.5)
	tween_in.tween_callback(func(): color_rect_fader.visible = false)

# Take an existing tween and add steps to fade the screen out.
func fade_out(tween_in: Tween):
	color_rect_fader.visible = true
	tween_in.tween_property(color_rect_fader, "color:a", 1.0, 0.25).from(0.0)

# Fade the screen out, reload the level and fade back in.
func reload_current_scene(hard_reset: bool) -> void:
	if not Lobby.IS_OFFLINE_MODE:
		print("Reloading is only supported in offline mode.")
		return
	before_reload.emit(hard_reset)
	# Store a reference to the player to pass its settings onto the next player.
	var player := main_player
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
		var new_player := main_player
		# Apply cached settings, but don't update position.
		new_player.view = view
		new_player.zoom = zoom
		hide_reticle(true)
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
	main_menu_container.focus()

func _on_main_menu_join_button_pressed():
	main_menu_container.visible = false
	join_game_container.visible = true
	join_game_container.focus()

func _on_main_menu_host_button_pressed():
	Lobby.IS_SERVER = true
	main_menu_container.visible = false
	fade_out_and_start_game()

func _on_join_menu_connect_button_pressed():
	Lobby.IS_CLIENT = true
	Lobby.HOST = join_game_container.host_field.text
	join_game_container.visible = false
	fade_out_and_start_game()


func _on_new_gallery_button_pressed():
	layout_config_container.visible = true
	pause_container.visible = false
	layout_config_container.focus()


func _on_layout_config_container_exit():
	layout_config_container.visible = false
	pause_container.visible = true
	resume_button.grab_focus()


func _on_new_layout_complete():
	_on_layout_config_container_exit()
	paused = false
