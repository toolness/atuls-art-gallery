extends Node

const PORT = 7000
const MAX_CONNECTIONS = 16

@onready var IS_HEADLESS := DisplayServer.get_name() == "headless"

var IS_CLIENT := false

var IS_SERVER := false

var IS_OFFLINE_MODE: bool:
    get:
        return !IS_CLIENT and !IS_SERVER

var HOST := "127.0.0.1"

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

func start():
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
