extends Window

class_name TeleportDialog

@onready var gallery_id_field: LineEdit = %GalleryIdField

@onready var submit_button: Button = %SubmitButton

signal teleport_requested(int)

func _ready():
	submit_button.disabled = true

func _unhandled_input(event: InputEvent):
	if event.is_action_pressed("ui_cancel"):
		close_requested.emit()
	if event.is_action_pressed("teleport_dialog"):
		close_requested.emit()
	if event.is_action_pressed("ui_accept"):
		_on_submit_button_pressed()

func show_and_focus_ui():
	show()
	gallery_id_field.grab_focus()

func _on_gallery_id_field_text_changed(new_text:String):
	submit_button.disabled = not parse_gallery_id(new_text).is_valid

func _on_gallery_id_field_text_submitted(new_text:String):
	var parsed := parse_gallery_id(new_text)
	if parsed.is_valid:
		teleport_requested.emit(parsed.id)

func _on_submit_button_pressed():
	_on_gallery_id_field_text_submitted(gallery_id_field.text)

class ParsedGalleryId:
	const MIN_ID = -9999
	const MAX_ID = 9999

	var id: int
	var is_valid: bool

func parse_gallery_id(value: String) -> ParsedGalleryId:
	var parsed := ParsedGalleryId.new()
	parsed.is_valid = false
	if value.is_valid_int():
		parsed.id = value.to_int()
		parsed.is_valid = parsed.id >= ParsedGalleryId.MIN_ID and parsed.id <= ParsedGalleryId.MAX_ID
	return parsed
