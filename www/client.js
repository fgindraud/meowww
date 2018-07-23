/******************************************************************************
 * Receive new message notifications.
 * Uses websockets in one direction only (server -> client).
 * Cannot use full duplex due to limited websocket api in rouille (server side).
 */
var messages = $('table');

function get_scrollbar () {
	if (document.scrollingElement) {
		return document.scrollingElement;
	} else if (document.body.scrollTop) {
		return document.body;
	} else {
		return null;
	}
}

function write_message(message) {
	messages.append($('<tr></tr>').append ($('<td></td>').text(message.nickname), $('<td></td>').text(message.content)));
	var scrollbar = get_scrollbar();
	scrollbar.scrollTop = scrollbar.scrollHeight; // Force scroll to bottom
}

var notifier = new WebSocket("ws://" + location.host + location.pathname + "/notify", "meowww");
notifier.onopen = function () { console.log("notifier open"); };
notifier.onerror = function (error) { console.log("notifier error: ", error); };
notifier.onmessage = function (message) {
	try {
		var json = JSON.parse(message.data);
		write_message(json);
	} catch (e) {
		console.log("notifier message is invalid: ", message.data);
	}
};

/******************************************************************************
 * Send POST requests for new messages.
 */
var form = $('form');
var submit_button = $('input[type="submit"]');
var input_nickname = $('input[name="nickname"]');
var input_content = $('input[name="content"]');

// FIXME temporarily disabled
//form.attr('onsubmit', 'return send_message()');
//
function send_message() {
	var message = {
		nickname: input_nickname.val(),
		content: input_content.val()
	};
	input_content.val('');

	write_message(message);
	return false; // Prevent form from being actually sent
}
