class_name SettingsContainer

extends MarginContainer

signal exit()

@export_category("FOV")
@export var min_fov: float = 20
@export var max_fov: float = 60
@export var default_fov: float = 60

@export_category("Mouse Sensitivity")
@export var very_low_sens: float = 0.00025
@export var low_sens: float = 0.0005
@export var medium_sens: float = 0.00075
@export var high_sens: float = 0.001
@export var very_high_sens: float = 0.00125

# A reference to the player for maintaining settings.
var _player: Player

@onready var fov_slider: HSlider = %FOVSlider
@onready var fov_value: Label = %FOVValue

@onready var sensitivity_dropdown: OptionButton = %SensitivityDropdown
@onready var shadow_panel: Control = %ShadowPanel
@onready var shadow_option_button: OptionButton = %ShadowOptionButton
@onready var ssaa_option_button: OptionButton = %SSAAOptionButton
@onready var msaa_option_button: OptionButton = %MSAAOptionButton
@onready var fullscreen_check_button: CheckButton = %FullscreenCheckButton
@onready var potato_mode_check_button: CheckButton = %PotatoModeCheckButton
@onready var gi_panel: Control = %GIPanel
@onready var gi_mode_check_button: CheckButton = %GIModeCheckButton
@onready var gallery_name_line_edit: LineEdit = %GalleryNameLineEdit

@onready var master_slider: HSlider = %MasterSlider
@onready var music_slider: HSlider = %MusicSlider
@onready var sfx_slider: HSlider = %SFXSlider

@onready var button: Button = $VBoxContainer/Button

@onready var is_fullscreen := DisplayServer.window_get_mode() == DisplayServer.WINDOW_MODE_EXCLUSIVE_FULLSCREEN

func focus() -> void:
	button.grab_focus()

func _ready() -> void:
	# These seem to trigger associated change events, but only if the setting is changed from its default...
	potato_mode_check_button.set_pressed(PersistedConfig.get_bool(PersistedConfig.POTATO_MODE, false))
	gi_mode_check_button.set_pressed(PersistedConfig.get_bool(PersistedConfig.GI_ENABLED, true))

	# Setting the text of a LineEdit doesn't trigger any events, so we'll have to call our event handler manually.
	var gallery_name := PersistedConfig.get_string(PersistedConfig.GALLERY_NAME, UserInterface.get_default_gallery_name())
	gallery_name_line_edit.text = gallery_name
	_on_gallery_name_line_edit_text_changed(gallery_name)

	fov_slider.min_value = min_fov
	fov_slider.max_value = max_fov
	fov_slider.value = default_fov
	
	# Default sensitivity to medium.
	sensitivity_dropdown.select(sensitivity_dropdown.selected)
	
	# Default shadow quality to medium.
	shadow_option_button.select(shadow_option_button.selected)
	_on_shadow_option_button_item_selected(shadow_option_button.selected)

	# Enable SSAA
	ssaa_option_button.select(ssaa_option_button.selected)
	_on_ssaa_option_button_item_selected(ssaa_option_button.selected)
	
	# 4x AA
	msaa_option_button.select(msaa_option_button.selected)
	_on_msaa_option_button_item_selected(msaa_option_button.selected)

	fullscreen_check_button.set_pressed_no_signal(is_fullscreen)

	# Set all audio  sliders to full volume
	master_slider.value = db_to_linear(AudioServer.get_bus_volume_db(AudioServer.get_bus_index("Master")))
	music_slider.value = db_to_linear(AudioServer.get_bus_volume_db(AudioServer.get_bus_index("Music")))
	sfx_slider.value = db_to_linear(AudioServer.get_bus_volume_db(AudioServer.get_bus_index("SFX")))
	
func _on_button_pressed() -> void:
	PersistedConfig.save()
	exit.emit()


func adjust_fov(delta: float) -> void:
	fov_slider.value = fov_slider.value + delta


func _on_fov_slider_value_changed(value: float) -> void:
	fov_value.text = str(value)
	# The player is then responsible for updating the second camera to the same.
	var camera = get_viewport().get_camera_3d()
	if is_instance_valid(camera):
		camera.fov = value


func _on_option_button_item_selected(index: int) -> void:
	if UserInterface.in_main_menu:
		return
	match index:
		0:
			_player.mouse_sensitivity = very_low_sens
		1:
			_player.mouse_sensitivity = low_sens
		2:
			_player.mouse_sensitivity = medium_sens
		3:
			_player.mouse_sensitivity = high_sens
		4:
			_player.mouse_sensitivity = very_high_sens

func toggle_fullscreen() -> void:
	_update_fullscreen_state(not is_fullscreen)

func _update_fullscreen_state(new_value: bool):
	is_fullscreen = new_value
	if is_fullscreen:
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_EXCLUSIVE_FULLSCREEN)
	else:
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_WINDOWED)

func _on_fullscreen_check_button_toggled(button_pressed: bool) -> void:
	_update_fullscreen_state(button_pressed)

func _on_potato_mode_check_button_toggled(toggled_on: bool):
	PersistedConfig.set_bool(PersistedConfig.POTATO_MODE, toggled_on)
	UserInterface.potato_mode = toggled_on
	shadow_panel.visible = not toggled_on
	gi_panel.visible = not toggled_on

func _on_gi_mode_check_button_toggled(toggled_on:bool):
	PersistedConfig.set_bool(PersistedConfig.GI_ENABLED, toggled_on)
	UserInterface.global_illumination = toggled_on

func _on_shadow_option_button_item_selected(index: int) -> void:
	match index:
		0:
			# Low.
			RenderingServer.positional_soft_shadow_filter_set_quality(RenderingServer.SHADOW_QUALITY_SOFT_VERY_LOW)
			RenderingServer.directional_soft_shadow_filter_set_quality(RenderingServer.SHADOW_QUALITY_SOFT_VERY_LOW)
		1:
			# Medium.
			RenderingServer.positional_soft_shadow_filter_set_quality(RenderingServer.SHADOW_QUALITY_SOFT_MEDIUM)
			RenderingServer.directional_soft_shadow_filter_set_quality(RenderingServer.SHADOW_QUALITY_SOFT_MEDIUM)
		2:
			# High.
			RenderingServer.positional_soft_shadow_filter_set_quality(RenderingServer.SHADOW_QUALITY_SOFT_ULTRA)
			RenderingServer.directional_soft_shadow_filter_set_quality(RenderingServer.SHADOW_QUALITY_SOFT_ULTRA)


func _on_msaa_option_button_item_selected(index: int) -> void:
	match index:
		0:
			get_viewport().msaa_3d = Viewport.MSAA_DISABLED
		1:
			
			get_viewport().msaa_3d = Viewport.MSAA_2X
		2:
			
			get_viewport().msaa_3d = Viewport.MSAA_4X
		3:
			
			get_viewport().msaa_3d = Viewport.MSAA_8X


func _on_ssaa_option_button_item_selected(index: int) -> void:
	match index:
		0:
			get_viewport().screen_space_aa = Viewport.SCREEN_SPACE_AA_DISABLED
		1:
			get_viewport().screen_space_aa = Viewport.SCREEN_SPACE_AA_FXAA


func _on_master_slider_value_changed(value: float) -> void:
	AudioServer.set_bus_volume_db(
		AudioServer.get_bus_index("Master"), 
		linear_to_db(value)
		)


func _on_music_slider_value_changed(value: float) -> void:
	AudioServer.set_bus_volume_db(
		AudioServer.get_bus_index("Music"), 
		linear_to_db(value)
		)


func _on_sfx_slider_value_changed(value: float) -> void:
	AudioServer.set_bus_volume_db(
		AudioServer.get_bus_index("SFX"), 
		linear_to_db(value)
		)


func _on_reload_button_pressed() -> void:
	if UserInterface.in_main_menu:
		# TODO: Really we shouldn't show this button when in the main menu?
		return
	UserInterface._on_resume_button_pressed()
	UserInterface.reload_current_scene(false)


# When the player node changes apply the latest gameplay settings.
func update_main_player(player_in: Player) -> void:
	_player = player_in
	_on_fov_slider_value_changed(fov_slider.value)
	_on_option_button_item_selected(sensitivity_dropdown.selected)
	


func _on_visibility_changed():
	if is_visible_in_tree():
		fullscreen_check_button.set_pressed_no_signal(is_fullscreen)


func _on_gallery_name_line_edit_text_changed(new_text:String) -> void:
	PersistedConfig.set_string(PersistedConfig.GALLERY_NAME, new_text)
	UserInterface.gallery_name = new_text
