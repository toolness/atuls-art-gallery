class_name Moma

extends Node3D

@onready var gallery: Node3D = $gallery

# Called when the node enters the scene tree for the first time.
func _ready():
	for child in gallery.get_children():
		if is_instance_of(child, MeshInstance3D):
			var aabb: AABB = child.get_aabb()
			if aabb.size.y > 0.1:
				# TODO: This is a wall, put art on it or something.
				pass
	print("CALLED _ready")

func boop():
	print("BOOP")


# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	pass
