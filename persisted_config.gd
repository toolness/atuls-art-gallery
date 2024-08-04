extends Node

var file := ConfigFile.new()

var ROOT_DIR: String

const SETTINGS_SECTION = "settings"
const POTATO_MODE = "potato_mode"
const GI_ENABLED = "global_illumination"
const GALLERY_FILTER = "gallery_filter"

const PLAYER_SECTION = "player"
const POSITION = "position"
const ROTATION = "rotation"
const TELEPORT_POSITION = "teleport_position"

func url() -> String:
	return ROOT_DIR + "settings.cfg"

func load_file():
	if file.load(url()) != OK:
		# This is fine, the settings file just doesn't exist yet.
		pass

func save():
	if file.save(url()) != OK:
		push_error("Saving " + url() + " failed.")

func set_bool(cfg_name: String, value: bool, section: String = SETTINGS_SECTION):
	file.set_value(section, cfg_name, value)

func get_bool(cfg_name: String, default: bool, section: String = SETTINGS_SECTION) -> bool:
	var value = file.get_value(section, cfg_name, default)
	if value is bool:
		return value
	return default

func set_string(cfg_name: String, value: String, section: String = SETTINGS_SECTION):
	file.set_value(section, cfg_name, value)

func get_string(cfg_name: String, default: String, section: String = SETTINGS_SECTION) -> String:
	var value = file.get_value(section, cfg_name, default)
	if value is String:
		return value
	return default

func set_vec3(cfg_name: String, value: Vector3, section: String = SETTINGS_SECTION):
	file.set_value(section, cfg_name, value)

func get_vec3(cfg_name: String, default: Vector3, section: String = SETTINGS_SECTION) -> Vector3:
	var value = file.get_value(section, cfg_name, default)
	if value is Vector3:
		return value
	return default

func delete_section(section: String):
	if file.has_section(section):
		file.erase_section(section)

# Called when the node enters the scene tree for the first time.
func _ready():
	if OS.has_feature("editor"):
		# Running from an editor binary.
		#
		# Store everything in a place that's convenient to access while developing,
		# relative to the project's root directory.
		#
		# If we change this dir, we will want to change where the CLI accesses things too.
		ROOT_DIR = "res://rust/cache/"
	else:
		# Running from an exported project.
		#
		# Store everything in the persistent user data directory:
		#
		#   https://docs.godotengine.org/en/stable/tutorials/io/data_paths.html#accessing-persistent-user-data-user
		ROOT_DIR = "user://"
	load_file()
