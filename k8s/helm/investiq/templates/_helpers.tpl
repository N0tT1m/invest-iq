{{/*
Expand the name of the chart.
*/}}
{{- define "investiq.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "investiq.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "investiq.labels" -}}
helm.sh/chart: {{ include "investiq.name" . }}-{{ .Chart.Version | replace "+" "_" }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: investiq
{{- end }}

{{/*
Selector labels for a given component
*/}}
{{- define "investiq.selectorLabels" -}}
app.kubernetes.io/name: {{ include "investiq.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}
