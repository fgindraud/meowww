function get_scrollbar () {
	if (document.scrollingElement) {
		return document.scrollingElement;
	} else if (document.body.scrollTop) {
		return document.body;
	} else {
		return null;
	}
}

var form = $('form');
var submit_button = $('input[type="submit"]');
var input_nickname = $('input[name="nickname"]');
var input_content = $('input[name="content"]');
var messages = $('table');

// FIXME temporarily disabled
//form.attr('onsubmit', 'return send_message()');

function write_message(message) {
	messages.append($('<tr></tr>').append ($('<td></td>').text(message.nickname), $('<td></td>').text(message.content)));
	var scrollbar = get_scrollbar();
	scrollbar.scrollTop = scrollbar.scrollHeight; // Force scroll to bottom
}

function send_message() {
	var message = {
		nickname: input_nickname.val(),
		content: input_content.val()
	};
	input_content.val('');

	write_message(message);
	return false; // Prevent form from being actually sent
}
