class_name Painting

extends Node3D


const PAINTING_SURFACE_IDX = 1


func init_with_size_and_color(width: float, height: float, color: Color) -> void:
	var painting: MeshInstance3D  = $painting.get_child(0)
	painting.set_scale(Vector3(width, height, 1.0))
	var material: StandardMaterial3D = painting.mesh.surface_get_material(PAINTING_SURFACE_IDX)
	var duplicate_material: StandardMaterial3D = material.duplicate()
	duplicate_material.albedo_color = color
	painting.set_surface_override_material(PAINTING_SURFACE_IDX, duplicate_material)


func init_with_met_object(object: MetObjects.MetObjectRecord) -> void:
	var painting: MeshInstance3D  = $painting.get_child(0)
	painting.set_scale(Vector3(object.width, object.height, 1.0))
	var material: StandardMaterial3D = painting.mesh.surface_get_material(PAINTING_SURFACE_IDX)
	var duplicate_material: StandardMaterial3D = material.duplicate()
	duplicate_material.albedo_color = Color.TRANSPARENT
	duplicate_material.albedo_texture = object.get_small_image_texture()
	painting.set_surface_override_material(PAINTING_SURFACE_IDX, duplicate_material)
