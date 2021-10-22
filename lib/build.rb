require 'liquid'

Liquid::Template.error_mode = :strict

class Build
    def initialize(*args)
        @variables = {}
        @template = nil
    end

    def set_content(content_str)
        @template = Liquid::Template.parse(content_str)
    end

    def set_var(name, value)
        @variables[name] = value
    end
    
    def render()
        @template.render(@variables)
    end
end