@tool

extends EditorScenePostImport

func _post_import(scene: Node) -> Node:
	var wall_data: Array[Dictionary] = []
	for child in scene.get_children():
		var wall := Moma.Wall.try_from_object(child, true)
		if wall:
			var datum := {
				"name": child.name,
				"width": wall.width,
				"height": wall.height
			}
			wall_data.push_back(datum)
	wall_data.sort_custom(_sort_walls_by_name)

	# We're going to reverse the order, because players actually navigate the gallery
	# from back to front, so we want a sparsely-populated gallery to be populated at
	# the earliest part of the player's journey.
	wall_data.reverse()

	var wall_data_json := JSON.stringify(wall_data, "  ")
	var source_filename := get_source_file()
	assert(source_filename.ends_with(".glb"), "Expected source filename to be GLB.")
	var json_filename := source_filename.replace(".glb", ".walls.json")
	var file := FileAccess.open(json_filename, FileAccess.WRITE)
	file.store_string(wall_data_json)
	file.close()
	print("Wrote ", json_filename, ".")
	return scene


func _sort_walls_by_name(a: Dictionary, b: Dictionary) -> bool:
	var a_name: String = a["name"]
	var b_name: String = b["name"]
	return a_name.naturalnocasecmp_to(b_name) < 0
