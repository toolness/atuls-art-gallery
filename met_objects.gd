extends Node


const MAX_REQUESTS_PER_FRAME = 10

const NULL_REQUEST_ID = 0

const GALLERY_ID_DB_OFFSET = 2

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
