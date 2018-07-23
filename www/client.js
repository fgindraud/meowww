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
var input_message = $('input[name="message"]');
var input_name = $('input[name="name"]');
var content = $('table');
var content_container = $('main');


form.attr('onsubmit', 'return send_message()');
submit_button.attr('disabled', false);

function write_message(nickname, message) {

	content.append($('<tr></tr>').append ($('<td></td>').text(nickname), $('<td></td>').text(message)));
	var scrollbar = get_scrollbar();
	scrollbar.scrollTop = scrollbar.scrollHeight; // Force scroll to bottom
}

for (i=0; i<30; i++) {
	write_message("John", "Blah " + i);
}

function send_message() {
	var nickname = input_name.val();
	var message = input_message.val();
	input_message.val('');

	write_message(nickname, message);
	return false; // Prevent form from being actually sent
}

