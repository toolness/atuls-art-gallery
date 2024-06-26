class_name InfiniteGallery

extends Node3D


@export var gallery_chunk_scene: PackedScene

@onready var player: Player = $Player

@onready var gallery_chunks: Array[Moma] = []

@export var spawned_example_scene: PackedScene

const GALLERY_CHUNK_WIDTH = 28

const GALLERY_SPAWN_RADIUS = 1


func get_gallery_id(x: float) -> int:
	return floori(x / float(GALLERY_CHUNK_WIDTH))


func sync_galleries() -> void:
	var middle_gallery_id := get_gallery_id(player.position.x)
	var min_gallery_id := middle_gallery_id - GALLERY_SPAWN_RADIUS
	var max_gallery_id := middle_gallery_id + GALLERY_SPAWN_RADIUS

	# Get rid of galleries that are far from the player.
	var new_gallery_chunks: Array[Moma] = []
	for gallery_chunk in gallery_chunks:
		var gallery_id := gallery_chunk.gallery_id
		if gallery_id < min_gallery_id or gallery_id > max_gallery_id:
			print("Despawning gallery with id ", gallery_id, " at ", gallery_chunk.position.x)
			remove_child(gallery_chunk)
		else:
			new_gallery_chunks.push_front(gallery_chunk)
	gallery_chunks = new_gallery_chunks

	# Spawn galleries that are near the player.
	for gallery_id: int in range(min_gallery_id, max_gallery_id + 1):
		var found := false
		for gallery_chunk in gallery_chunks:
			if gallery_chunk.gallery_id == gallery_id:
				found = true
				break
		if not found:
			var instance: Moma = gallery_chunk_scene.instantiate()
			instance.position.x = gallery_id * GALLERY_CHUNK_WIDTH
			print("Spawning new gallery with id ", gallery_id, " at ", instance.position.x)
			add_child(instance)
			gallery_chunks.push_front(instance)
			instance.init(gallery_id, player)


# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	# Even if we set the reference gallery to not be visible, raycasts still intersect with
	# it, which is weird, so just remove it.
	remove_child(%Moma_for_reference_only)

	if Lobby.IS_SERVER:
		_spawn_multiplayer_example_objects()

	sync_galleries()


func _spawn_multiplayer_example_objects():
	# These just exist to try out Godot's MultiplayerSpawner and MultiplayerSynchronizer classes.
	var rng := RandomNumberGenerator.new()
	for i in range(10):
		var example: Node3D = spawned_example_scene.instantiate()
		var example_position := Vector3(rng.randf_range(28.0, 38.0), 1.0, rng.randf_range(1.0, 7.0))
		example.translate(example_position)
		add_child(example, true)


func _process(_delta) -> void:
	sync_galleries()
