extends Area3D

@export_enum("honey_mustard", "ranch_shield") var powerup_type: String = "honey_mustard"
@export var spin_speed: float = 2.0
@export var bob_amplitude: float = 0.15

var start_y: float = 0.0
var time_passed: float = 0.0

func _ready() -> void:
	start_y = position.y
	body_entered.connect(_on_body_entered)
	var model_path := (
		"res://assets/models/honey_mustard_cup.glb"
		if powerup_type == "honey_mustard"
		else "res://assets/models/ranch_shield.glb"
	)
	if ResourceLoader.exists(model_path):
		var packed := load(model_path) as PackedScene
		if packed:
			add_child(packed.instantiate())
			var fallback := get_node_or_null("FallbackMesh")
			if fallback:
				fallback.visible = false

func _process(delta: float) -> void:
	time_passed += delta
	rotate_y(spin_speed * delta)
	position.y = start_y + sin(time_passed * 3.0) * bob_amplitude

func _on_body_entered(body: Node3D) -> void:
	if not body.is_in_group("player"):
		return
	if powerup_type == "honey_mustard" and body.has_method("activate_honey_mustard"):
		body.activate_honey_mustard(10.0)
		queue_free()
	elif powerup_type == "ranch_shield" and body.has_method("activate_ranch_shield"):
		body.activate_ranch_shield()
		queue_free()
