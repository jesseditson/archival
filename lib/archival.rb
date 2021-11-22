# frozen_string_literal: true

require 'liquid'
require 'tags/layout'
require 'tags/asset'

module Archival
  # Main Archival module. See https://archival.dev for docs.
end

Liquid::Template.error_mode = :strict
Liquid::Template.register_tag('layout', Layout)
Liquid::Template.register_tag('asset', Asset)

require 'archival/version'
require 'archival/template_array'
require 'archival/logger'
require 'archival/config'
require 'archival/markdown_renderer'
require 'archival/helper_server'
require 'archival/parser'
require 'archival/builder'
require 'archival/listen'
