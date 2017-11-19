{% for i in foo.shoo %}
{{ i.one }}
{% endfor %}
{% for i in list %}
{{ i }}
{% endfor %}
{% if local is defined %}
{{ local }}
{% endif %}
{% if default is defined %}
{{ default }}
{% endif %}
{% if custom is defined %}
{{ custom }}
{% endif %}
