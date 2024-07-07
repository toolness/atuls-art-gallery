extends CenterContainer

class_name LayoutConfigContainer

signal exit()

signal new_layout_complete()

@onready var dense_layout_checkbox: CheckBox = %DenseLayoutCheckBox
@onready var do_layout_button: Button = %DoLayoutButton

# Called when the node enters the scene tree for the first time.
func _ready():
	pass # Replace with function body.


func _on_back_button_pressed():
	exit.emit()


func focus():
	dense_layout_checkbox.grab_focus()


func _on_do_layout_button_pressed():
	var use_dense_layout := dense_layout_checkbox.button_pressed

	# Disable this button while the layout is being generated, so the
	# user can't click on it multiple times and cause chaos. This is also
	# effectively a kind of low-fidelity loading indicator, letting the
	# user know something is happening.
	do_layout_button.disabled = true

	# We need to un-pause the tree so we can poll for the response to our
	# request to the Rust worker thread--otherwise the subsequent `await`
	# will never return.
	get_tree().paused = false

	await MetObjects.layout(use_dense_layout)
	do_layout_button.disabled = false
	new_layout_complete.emit()
