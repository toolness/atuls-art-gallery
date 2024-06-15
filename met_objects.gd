extends Node


const ENABLE_MET_OBJECTS := true

const ENABLE_DB_MET_OBJECTS := true

const MAX_OBJECT_ATTEMPTS = 10

const MAX_REQUESTS_PER_FRAME = 10

const NULL_REQUEST_ID = 0

const GALLERY_ID_DB_OFFSET = 2

var keyed_met_objects := {}

var unused_met_objects: Array[MetObject] = []

var requests = {}


class MetObjectRequest:
	var response: MetObject
	signal responded


class MetObjectsRequest:
	var response: Array[MetObject]
	signal responded


func get_met_objects_for_gallery_wall(gallery_id: int, wall_id: String) -> Array[MetObject]:
	var db_gallery_id := gallery_id + GALLERY_ID_DB_OFFSET
	if db_gallery_id < 0:
		return []
	var request := MetObjectsRequest.new()
	var request_id := RustMetObjects.get_met_objects_for_gallery_wall(db_gallery_id, wall_id)
	if request_id == NULL_REQUEST_ID:
		# Oof, something went wrong.
		return []
	requests[request_id] = request
	await request.responded
	return request.response


func _get_next_object() -> MetObject:
	var request := MetObjectRequest.new()
	var request_id := RustMetObjects.next_csv_record()
	if request_id == NULL_REQUEST_ID:
		# Oof, something went wrong.
		return null
	requests[request_id] = request
	await request.responded
	return request.response


func _try_to_get_new_met_object(max_width: float, max_height: float) -> MetObject:
	for i in range(MAX_OBJECT_ATTEMPTS):
		var met_object := await _get_next_object()
		if not met_object:
			return null
		if met_object.can_fit_in(max_width, max_height):
			return met_object
		else:
			# The art is too wide/tall to fit on the wall.
			unused_met_objects.push_back(met_object)
	return null


func _try_to_get_unused_met_object(max_width: float, max_height: float) -> MetObject:
	for met_object in unused_met_objects:
		if met_object.can_fit_in(max_width, max_height):
			unused_met_objects.erase(met_object)
			return met_object
	return null


func try_to_get_next_object(key: String, max_width: float, max_height: float) -> MetObject:
	if not ENABLE_MET_OBJECTS:
		return null
	if not keyed_met_objects.has(key):
		var met_object := _try_to_get_unused_met_object(max_width, max_height)
		if not met_object:
			met_object = await _try_to_get_new_met_object(max_width, max_height)
			if not met_object:
				print("Unable to find object to fit in ", max_width, " x ", max_height)
				return null
		keyed_met_objects[key] = met_object
	return keyed_met_objects[key]


func _process(_delta) -> void:
	for i in range(MAX_REQUESTS_PER_FRAME):
		var obj := RustMetObjects.poll()
		if not obj:
			return
		if not requests.has(obj.request_id):
			print("Warning: request #", obj.request_id, " does not exist.")
			return
		var request = requests[obj.request_id]
		requests.erase(obj.request_id)
		if request is MetObjectRequest:
			var r: MetObjectRequest = request
			r.response = obj.take_optional_met_object()
			r.responded.emit()
		elif request is MetObjectsRequest:
			var r: MetObjectsRequest = request
			r.response = obj.take_met_objects()
			r.responded.emit()
		else:
			assert(false, "Unknown request type, cannot fill response")
