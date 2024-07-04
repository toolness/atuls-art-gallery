extends CenterContainer
class_name JoinGameContainer

@onready var host_field: LineEdit = %HostField

@onready var connect_button: Button = %ConnectButton

@onready var back_button: Button = %BackButton


func focus():
	host_field.grab_focus()


# Called when the node enters the scene tree for the first time.
func _ready():
	pass # Replace with function body.
