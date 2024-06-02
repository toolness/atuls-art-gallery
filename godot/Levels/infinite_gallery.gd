class_name InfiniteGallery

extends Node3D


@export var gallery_chunk_scene: PackedScene

@onready var player: Player = $Player

@onready var gallery_chunks: Array[Moma] = []

const GALLERY_CHUNK_WIDTH = 28


func get_gallery_id(x: float) -> int:
	return floori(x / float(GALLERY_CHUNK_WIDTH))


func sync_galleries() -> void:
	var middle_gallery_id := get_gallery_id(player.position.x)
	var min_gallery_id := middle_gallery_id - 1
	var max_gallery_id := middle_gallery_id + 1

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
	for gallery_id: int in [min_gallery_id, middle_gallery_id, max_gallery_id]:
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
			instance.init(gallery_id)


func _loop_stuff():
	while true:
		RustMetObjects.next()
		var obj: String
		while not obj:
			obj = RustMetObjects.poll()
			await get_tree().process_frame
		print("GOT OBJ", obj)
		await get_tree().create_timer(2).timeout


# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	print("RustMetObjects.add(1, 2) = ", RustMetObjects.add(1, 2))
	sync_galleries()
	_loop_stuff()


func _process(_delta) -> void:
	sync_galleries()
