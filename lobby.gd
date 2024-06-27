extends Node

const PORT = 7000
const MAX_CONNECTIONS = 16

@onready var IS_HEADLESS := DisplayServer.get_name() == "headless"

var IS_CLIENT := false

var IS_SERVER := false

var IS_OFFLINE_MODE := false

var HOST := "127.0.0.1"

var DISABLE_INITIAL_MOUSE_CAPTURE := false

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
        IS_CLIENT = true
    if args.has("--server"):
        IS_SERVER = true
    if args.has("--host"):
        HOST = args.get("--host")
    if args.has("--nocap"):
        DISABLE_INITIAL_MOUSE_CAPTURE = true
    if IS_CLIENT and IS_SERVER:
        OS.alert("Cannot be server and client simultaneously!")
        get_tree().quit(1)
    IS_OFFLINE_MODE = !IS_CLIENT and !IS_SERVER

func _on_connected_to_server():
    var peer_id := multiplayer.get_unique_id()
    print("Connected to server with peer ID ", peer_id, ".")

func _on_connection_failed():
    print("Connection failed.")

func _on_server_disconnected():
    print("Server disconnected.")

func _on_peer_connected(id):
    print("Peer ", id, " connected.")

func _on_peer_disconnected(id):
    print("Peer ", id, " disconnected.")

func _ready():
    _parse_cmdline_args()

    multiplayer.connected_to_server.connect(_on_connected_to_server)
    multiplayer.connection_failed.connect(_on_connection_failed)
    multiplayer.server_disconnected.connect(_on_server_disconnected)
    multiplayer.peer_connected.connect(_on_peer_connected)
    multiplayer.peer_disconnected.connect(_on_peer_disconnected)

    if IS_SERVER:
        var peer := ENetMultiplayerPeer.new()
        var error := peer.create_server(PORT, MAX_CONNECTIONS)
        if error:
            print("Failed to create server: ", error)
            return
        print("Started server on port ", PORT, ".")
        multiplayer.multiplayer_peer = peer
    elif IS_CLIENT:
        var peer := ENetMultiplayerPeer.new()
        var error := peer.create_client(HOST, PORT)
        if error:
            print("Failed to create client: ", error)
            return
        print("Connecting to server on ", HOST, ":", PORT, ".")
        multiplayer.multiplayer_peer = peer
