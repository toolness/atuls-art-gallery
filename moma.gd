class_name Moma

extends Node3D

@onready var gallery: Node3D = $gallery

const MIN_WALL_MOUNT_SIZE = 2

var gallery_id: int

# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	for child in gallery.get_children():
		if is_instance_of(child, MeshInstance3D):
			var mesh: MeshInstance3D = child
			var aabb: AABB = mesh.get_aabb()
			if aabb.size.y < 0.1:
				# This is a floor or ceiling, it has no height.
				continue
			if aabb.size.x > MIN_WALL_MOUNT_SIZE:
				# We can mount art along the x-axis.
				pass
			elif aabb.size.y > MIN_WALL_MOUNT_SIZE:
				# We can mount art along the y-axis.
				pass
			else:
				pass


func init(new_gallery_id: int) -> void:
	gallery_id = new_gallery_id
	print("Initializing gallery ", gallery_id)
