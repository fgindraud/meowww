/******************************************************************************
 * Content manipulation.
 */
function get_scrollbar () {
	if (document.scrollingElement) {
		return document.scrollingElement;
	} else if (document.body.scrollTop) {
		return document.body;
	} else {
		return null;
	}
}
function scroll_to_bottom () {
	var scrollbar = get_scrollbar();
	scrollbar.scrollTop = scrollbar.scrollHeight; // Force scroll to bottom
}

var messages = $('table');

function write_message(message) {
	messages.append($('<tr></tr>').append (
		$('<td></td>').text(message.nickname),
		$('<td></td>').text(message.content)));
	scroll_to_bottom();
}
function error_message(text) {
	messages.append($('<tr class="error"></tr>').append(
		$('<td></td>').text("Error"),
		$('<td></td>').text(text)));
	scroll_to_bottom();
}

/******************************************************************************
 * Receive new message notifications.
 * Uses websockets in one direction only (server -> client).
 * Cannot use full duplex due to limited websocket api in rouille (server side).
 */
var notifier = new WebSocket("ws://" + location.host + location.pathname + "/notify", "meowww");
notifier.onerror = function (error) {
	error_message("Notification connection error: " + error);
};
notifier.onclose = function (event) {
	error_message("Notification connection closed unexpectedly");
};
notifier.onmessage = function (message) {
	try {
		var json = JSON.parse(message.data);
		write_message(json);
	} catch (e) {
		error_message("Invalid notification message");
	}
};

/******************************************************************************
 * Send POST requests for new messages.
 */
var input_nickname = $('input[name="nickname"]');
var input_content = $('input[name="content"]');

function send_message() {
	var message = {
		nickname: input_nickname.val(),
		content: input_content.val()
	};
	input_content.val('');

	$.ajax({
		type: "POST",
		data: message,
		error: function (xhr, status, error) {
			error_message("Failed to send message");
		}
	});
	return false;
}

$('form').attr('onsubmit', 'return send_message()');
