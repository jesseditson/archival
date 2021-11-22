# frozen_string_literal: true

Gem::Specification.new do |s|
  s.name = 'archival'
  s.version     = '0.0.8'
  s.summary     = 'An incredibly simple CMS for durable websites'
  s.description = 'https://jesseditson.com/the-simplest-cms-part-1'
  s.authors     = ['Jesse Ditson']
  s.email       = 'jesse.ditson@gmail.com'
  s.homepage    = 'https://archival.dev'
  s.license = 'Unlicense'
  s.required_ruby_version = '>= 2.5'

  # Specify which files should be added to the gem when it is released.
  # The `git ls-files -z` loads the files in the RubyGem that have been added into git.
  s.files = Dir.chdir(File.expand_path(__dir__)) do
    `git ls-files -z`.split("\x0").reject do |f|
      (f == __FILE__) || f.match(%r{\A(?:(?:test|spec|features)/|\.(?:git|travis|circleci)|appveyor)})
    end
  end
  s.bindir        = 'exe'
  s.executables   = 'archival'
  s.require_paths = ['lib']

  s.add_dependency 'liquid', '~> 5.1.0'
  s.add_dependency 'listen', '~> 3.7.0'
  s.add_dependency 'redcarpet', '~> 3.5.1'
  s.add_dependency 'tomlrb', '~> 2.0.1'

  s.metadata = {
    'rubygems_mfa_required' => 'true'
  }
end
