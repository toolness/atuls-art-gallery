extends CenterContainer

class_name LayoutConfigContainer

signal exit()

signal new_layout_complete()

@onready var dense_layout_checkbox: CheckBox = %DenseLayoutCheckBox
@onready var filter_line_edit: LineEdit = %FilterLineEdit
@onready var filter_results_label: Label = %FilterResultsLabel
@onready var do_layout_button: Button = %DoLayoutButton
@onready var back_button: Button = %BackButton

# Called when the node enters the scene tree for the first time.
func _ready():
	filter_line_edit.text_changed.connect(_on_filter_text_changed)


func _on_filter_text_changed(filter: String):
	if not filter:
		filter_results_label.text = ""
		return
	var count := await MetObjects.count_met_objects(filter)
	if filter_line_edit.text != filter:
		# The player's input has changed, so our count is no
		# longer accurate.
		return
	filter_results_label.text = str(count) + " artworks match your filter."


func _on_back_button_pressed():
	get_tree().paused = true
	exit.emit()


func focus():
	# We need to un-pause the tree so we can poll for the response to our
	# requests to the Rust worker thread--otherwise any `await`s we do
	# will never return.
	get_tree().paused = false

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

	await MetObjects.layout(filter, use_dense_layout)
	do_layout_button.disabled = false
	back_button.disabled = false
	new_layout_complete.emit()
