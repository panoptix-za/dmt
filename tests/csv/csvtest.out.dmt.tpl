{% for k,v in servers %}
{{ k }}
{% endfor %}

{% for s in hosts %}
{{ s.host }}
{% endfor %}

