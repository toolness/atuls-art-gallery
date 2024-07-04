extends CenterContainer
class_name MainMenuContainer

@onready var start_button: Button = %StartButton

@onready var join_button: Button = %JoinButton

@onready var host_button: Button = %HostButton

@onready var settings_button: Button = %SettingsButton

@onready var quit_button: Button = %QuitButton


func focus():
	start_button.grab_focus()


# Called when the node enters the scene tree for the first time.
func _ready():
	pass # Replace with function body.
