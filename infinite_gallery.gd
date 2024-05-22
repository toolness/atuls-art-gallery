extends Node3D


@onready var gallery_chunk = preload("res://moma.tscn")

var GALLERY_CHUNK_WIDTH = 28

# Called when the node enters the scene tree for the first time.
func _ready():
	for i in range(3):
		var instance: Node = gallery_chunk.instantiate()
		instance.position.x = i * GALLERY_CHUNK_WIDTH
		add_child(instance)



# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	pass
