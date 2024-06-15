extends Node


const ENABLE_MET_OBJECTS := true

const MAX_OBJECT_ATTEMPTS = 10

const MAX_REQUESTS_PER_FRAME = 10

const NULL_REQUEST_ID = 0

var keyed_met_objects := {}

var unused_met_objects: Array[MetObject] = []

var requests = {}


class MetObjectRequest:
	var response: MetObject
	signal responded


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
		var request: MetObjectRequest = requests[obj.request_id]
		requests.erase(obj.request_id)
		request.response = obj.get()
		request.responded.emit()
