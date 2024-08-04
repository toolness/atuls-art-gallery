extends CenterContainer

class_name LayoutConfigContainer

signal exit()

signal new_layout_complete()

@onready var dense_layout_checkbox: CheckBox = %DenseLayoutCheckBox
@onready var filter_line_edit: LineEdit = %FilterLineEdit
@onready var filter_results_label: Label = %FilterResultsLabel
@onready var do_layout_button: Button = %DoLayoutButton
@onready var back_button: Button = %BackButton

const DEBOUNCE_SECS = 0.250

var _latest_filter_text_version := 0

# Called when the node enters the scene tree for the first time.
func _ready():
	filter_line_edit.text_changed.connect(_on_filter_text_changed)
	filter_line_edit.text = PersistedConfig.get_string(PersistedConfig.GALLERY_FILTER, "")
	filter_line_edit.select_all()


func _on_filter_text_changed(filter: String):
	_latest_filter_text_version += 1
	var version := _latest_filter_text_version
	if not filter:
		filter_results_label.text = ""
		return
	await get_tree().create_timer(DEBOUNCE_SECS).timeout
	if !is_inside_tree() || version != _latest_filter_text_version:
		# The player left the UI, or their input has changed, so don't
		# process the filter.
		return
	var count := await ArtObjects.count_art_objects(filter)
	if !is_inside_tree() || version != _latest_filter_text_version:
		# The player left the UI, or their input has changed, so our count is no
		# longer accurate.
		return
	filter_results_label.text = str(count) + " artworks match your filter."


func _on_back_button_pressed():
	exit.emit()


func focus():
	_on_filter_text_changed(filter_line_edit.text)
	filter_line_edit.grab_focus()


func _on_do_layout_button_pressed():
	var use_dense_layout := dense_layout_checkbox.button_pressed
	var filter := filter_line_edit.text

	# Disable this button while the layout is being generated, so the
	# user can't click on it multiple times and cause chaos. This is also
	# effectively a kind of low-fidelity loading indicator, letting the
	# user know something is happening.
	do_layout_button.disabled = true
	back_button.disabled = true

	await ArtObjects.layout(filter, use_dense_layout)
	do_layout_button.disabled = false
	back_button.disabled = false
	PersistedConfig.set_string(PersistedConfig.GALLERY_FILTER, filter)
	PersistedConfig.save()
	new_layout_complete.emit()
