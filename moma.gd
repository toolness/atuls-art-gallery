class_name Moma

extends Node3D

@onready var gallery: Node3D = $gallery

var gallery_id: int

# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	for child in gallery.get_children():
		if is_instance_of(child, MeshInstance3D):
			var aabb: AABB = child.get_aabb()
			if aabb.size.y > 0.1:
				# TODO: This is a wall, put art on it or something.
				pass
	print("CALLED _ready")


func init(new_gallery_id: int) -> void:
	gallery_id = new_gallery_id
	print("Initializing gallery ", gallery_id)
