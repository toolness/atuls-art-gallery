extends Node


const MAX_REQUESTS_PER_FRAME = 10

const NULL_REQUEST_ID = 0

var requests = {}


class MetObjectsRequest:
	var response: Array[MetObject]
	signal responded


class ImageRequest:
	var response: Image
	signal responded


func fetch_small_image(object_id: int) -> Image:
	var request := ImageRequest.new()
	var request_id := RustMetObjects.fetch_small_image(object_id)
	if request_id == NULL_REQUEST_ID:
		# Oof, something went wrong.
		return null
	requests[request_id] = request
	await request.responded
	return request.response


func get_met_objects_for_gallery_wall(gallery_id: int, wall_id: String) -> Array[MetObject]:
	var request := MetObjectsRequest.new()
	var request_id := RustMetObjects.get_met_objects_for_gallery_wall(gallery_id, wall_id)
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
		if request is ImageRequest:
			var r: ImageRequest = request
			r.response = obj.take_optional_image()
			r.responded.emit()
		elif request is MetObjectsRequest:
			var r: MetObjectsRequest = request
			r.response = obj.take_met_objects()
			r.responded.emit()
		else:
			assert(false, "Unknown request type, cannot fill response")
