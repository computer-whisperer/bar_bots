local widget = widget
function widget:GetInfo()
	return { name = "Arena Quit", desc = "quits the headless client as soon as the game is over", layer = 0, enabled = true }
end
function widget:GameOver()
	Spring.Echo("<arena quit> game over, quitting")
	Spring.SendCommands("quitforce")
end
