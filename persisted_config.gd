extends Node

var file := ConfigFile.new()

var ROOT_DIR: String

class Setting:
	var section_name: String
	var name: String

	static func create(new_section_name: String, new_name: String) -> Setting:
		var setting := Setting.new()
		setting.section_name = new_section_name
		setting.name = new_name
		return setting

const VIDEO_SECTION = "settings"
const GALLERY_SECTION = "gallery"
const PLAYER_SECTION = "player"

var POTATO_MODE := Setting.create(VIDEO_SECTION, "potato_mode")
var GI_ENABLED := Setting.create(VIDEO_SECTION, "global_illumination")
var GALLERY_FILTER := Setting.create(GALLERY_SECTION, "gallery_filter")

var PLAYER_POSITION := Setting.create(PLAYER_SECTION, "position")
var PLAYER_ROTATION := Setting.create(PLAYER_SECTION, "rotation")
var PLAYER_TELEPORT_POSITION := Setting.create(PLAYER_SECTION, "teleport_position")

func url() -> String:
	return ROOT_DIR + "settings.cfg"

func load_file():
	if file.load(url()) != OK:
		# This is fine, the settings file just doesn't exist yet.
		pass

func save():
	if file.save(url()) != OK:
		push_error("Saving " + url() + " failed.")

func set_bool(setting: Setting, value: bool):
	file.set_value(setting.section_name, setting.name, value)

func get_bool(setting: Setting, default: bool) -> bool:
	var value = file.get_value(setting.section_name, setting.name, default)
	if value is bool:
		return value
	return default

func set_string(setting: Setting, value: String):
	file.set_value(setting.section_name, setting.name, value)

func get_string(setting: Setting, default: String) -> String:
	var value = file.get_value(setting.section_name, setting.name, default)
	if value is String:
		return value
	return default

func set_vec3(setting: Setting, value: Vector3):
	file.set_value(setting.section_name, setting.name, value)

func get_vec3(setting: Setting, default: Vector3) -> Vector3:
	var value = file.get_value(setting.section_name, setting.name, default)
	if value is Vector3:
		return value
	return default

func delete_section(section: String):
	if file.has_section(section):
		file.erase_section(section)

func delete_player_settings():
	print("Deleting player settings.")
	delete_section(PersistedConfig.PLAYER_SECTION)
	save()

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
