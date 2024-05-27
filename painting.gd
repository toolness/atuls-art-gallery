class_name Painting

extends Node3D


func init(width: float, height: float, color: Color) -> void:
	var painting: MeshInstance3D  = $painting.get_child(0)
	painting.set_scale(Vector3(width, height, 1.0))
	# TODO: Set color.
	var _unused_color = color
