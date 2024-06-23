extends Node

const PORT = 7000
const MAX_CONNECTIONS = 1

@onready var IS_HEADLESS := DisplayServer.get_name() == "headless"

var IS_CLIENT := false

var IS_SERVER := false

func _parse_cmdline_args():
    for arg in OS.get_cmdline_args():
        if arg == "--client":
            IS_CLIENT = true
        elif arg == "--server":
            IS_SERVER = true
    if IS_CLIENT and IS_SERVER:
        OS.crash("Cannot be server and client simultaneously!")

func _on_connected_to_server():
    var peer_id := multiplayer.get_unique_id()
    print("Connected to server with peer ID ", peer_id, ".")

func _on_connection_failed():
    print("Connection failed.")
    multiplayer.multiplayer_peer = null

func _on_server_disconnected():
    print("Server disconnected.")
    multiplayer.multiplayer_peer = null

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
        var host := "127.0.0.1"
        var error := peer.create_client(host, PORT)
        if error:
            print("Failed to create client: ", error)
            return
        print("Connecting to server on ", host, ":", PORT, ".")
        multiplayer.multiplayer_peer = peer
    else:
        multiplayer.multiplayer_peer = null
