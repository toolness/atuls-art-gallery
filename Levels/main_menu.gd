extends Node2D


var auto_start := false


func _get_cmdline_args_dict() -> Dictionary:
	var cmdline_args := OS.get_cmdline_args()
	if OS.has_feature("editor") and cmdline_args.size() == 1 and cmdline_args[0].find("res://") == 0:
		# We were started from the GUI editor, try to load command-line args
		# from a JSON file.
		const EDITOR_CMDLINE_ARGS_FILE = "res://editor-cmdline-args.json"
		if FileAccess.file_exists(EDITOR_CMDLINE_ARGS_FILE):
			var contents = JSON.parse_string(FileAccess.get_file_as_string(EDITOR_CMDLINE_ARGS_FILE))
			if contents is Array:
				cmdline_args = contents
				print("Loaded command line args from ", EDITOR_CMDLINE_ARGS_FILE, ": ", cmdline_args)
	var arguments = {}
	for argument in cmdline_args:
		if argument.find("=") > -1:
			var key_value = argument.split("=")
			arguments[key_value[0]] = key_value[1]
		else:
			# Options without an argument will be present in the dictionary,
			# with the value set to an empty string.
			arguments[argument] = ""
	return arguments


func _parse_cmdline_args():
	var args = _get_cmdline_args_dict()
	if args.has("--client"):
		Lobby.IS_CLIENT = true
	if args.has("--server"):
		Lobby.IS_SERVER = true
	if args.has("--host"):
		Lobby.HOST = args.get("--host")
	if args.has("--nocap"):
		UserInterface.DISABLE_INITIAL_MOUSE_CAPTURE = true
	if Lobby.IS_CLIENT and Lobby.IS_SERVER:
		OS.alert("Cannot be server and client simultaneously!")
		get_tree().quit(1)
	if Lobby.IS_CLIENT or Lobby.IS_SERVER or args.has("--autostart"):
		auto_start = true


# Called when the node enters the scene tree for the first time.
func _ready():
	_parse_cmdline_args()

	if auto_start:
		# Calling deferred avoids a
		# "Parent node is busy adding/removing children" error.
		UserInterface.start_game.call_deferred()
	else:
		UserInterface.show_main_menu()
