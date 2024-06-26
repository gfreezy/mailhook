# smtplib provides functionality to send emails using SMTP.
import smtplib

# MIMEMultipart send emails with both text content and attachments.
from email.mime.multipart import MIMEMultipart

# MIMEText for creating body of the email message.
from email.mime.text import MIMEText

# MIMEApplication attaching application-specific data (like CSV files) to email messages.
from email.mime.application import MIMEApplication

# SMTP server configuration
smtp_server = "harbor.allsunday.io"
smtp_server = "localhost"
smtp_port = 25

subject = "Email Subject"
body = "This is the body of the text message"
sender_email = "sender@google.com"
recipient_email = "oc_3afec1ef7b7a16acacb15280078d4780@mail.allsunday.io"


# MIMEMultipart() creates a container for an email message that can hold
# different parts, like text and attachments and in next line we are
# attaching different parts to email container like subject and others.
message = MIMEMultipart()
message["Subject"] = subject
message["From"] = sender_email
message["To"] = recipient_email
body_part = MIMEText(body)
message.attach(body_part)

path_to_file = "/Users/feichao/Downloads/fp.pdf"
# section 1 to attach file
with open(path_to_file, "rb") as file:
    # Attach the file with filename to the email
    message.attach(MIMEApplication(file.read(), Name="fp.pdf"))
with open(path_to_file, "rb") as file:
    # Attach the file with filename to the email
    message.attach(MIMEApplication(file.read(), Name="fp.pdf"))

# secction 2 for sending email
with smtplib.SMTP(smtp_server, smtp_port) as server:
    server.sendmail(sender_email, recipient_email, message.as_string())
