use crate::joypaths;
use crate::Team;
use crate::TeamLock;
use gilrs;
use software_joystick::*;
use strum::IntoEnumIterator;

pub struct Outjoys {
    pub outjoys: Vec<Outjoy>,
}

pub struct Outjoy {
    team: Team,
    joy: Joystick,
}

fn inbutton_to_outbutton(b: &crate::injoy::NamedButton) -> software_joystick::Button {
    use crate::injoy::NamedButton;
    match b {
        NamedButton::X => software_joystick::Button::RightNorth,
        NamedButton::A => software_joystick::Button::RightEast,
        NamedButton::B => software_joystick::Button::RightSouth,
        NamedButton::Y => software_joystick::Button::RightWest,
        NamedButton::L => software_joystick::Button::L1,
        NamedButton::R => software_joystick::Button::R1,
        NamedButton::Start => software_joystick::Button::RightSpecial,
        NamedButton::Select => software_joystick::Button::LeftSpecial,
    }
}

fn inaxis_to_outaxis(a: &crate::injoy::NamedAxis) -> software_joystick::Axis {
    use crate::injoy::NamedAxis;
    match a {
        NamedAxis::Xright => software_joystick::Axis::X,
        NamedAxis::Yup => software_joystick::Axis::Y,
    }
}

impl Outjoy {
    pub fn new(team: Team, index: u32) -> Self {
        let joy = Joystick::new(format!("Buster{}", index)).unwrap();
        Self { team, joy }
    }

    fn mutate_team(&mut self, team: Team) {
        self.team = team;
    }

    fn inaxis_to_letter(a: &crate::injoy::NamedAxis, f: f32) -> Option<String> {
        use crate::injoy::NamedAxis;
        match a {
            NamedAxis::Xright => match f {
                f if f > 0.1 => Some(">".to_string()),
                f if f < -0.1 => Some("<".to_string()),
                _ => None,
            },
            NamedAxis::Yup => match f {
                f if f > 0.1 => Some("^".to_string()),
                f if f < -0.1 => Some("v".to_string()),
                _ => None,
            },
        }
    }

    fn inbutton_to_letter(b: &crate::injoy::NamedButton) -> String {
        use crate::injoy::NamedButton;
        match b {
            NamedButton::X => "X".to_string(),
            NamedButton::A => "A".to_string(),
            NamedButton::B => "B".to_string(),
            NamedButton::Y => "Y".to_string(),
            NamedButton::L => "L".to_string(),
            NamedButton::R => "R".to_string(),
            NamedButton::Start => "t".to_string(),
            NamedButton::Select => "e".to_string(),
        }
    }

    fn update_axes<'b, 'c, 'd, 'e>(&self, context: &'d mut UpdateContext<'b, 'c, 'e>) {
        use crate::injoy::NamedAxis;

        let mut fb_team = None;
        for team in context.feedback.teams.iter_mut() {
            if self.team.name == team.team_name {
                fb_team = Some(team);
                break;
            }
        }

        let lefties = ["<".to_string(), ">".to_string()];
        let upies = ["^".to_string(), "v".to_string()];

        for (i, inaxis) in crate::injoy::NamedAxis::iter().enumerate() {
            let mut sum = 0 as f32;
            let mut count = 0;

            let out_axis = inaxis_to_outaxis(&inaxis);

            let clearem = match inaxis {
                NamedAxis::Xright => &lefties,
                NamedAxis::Yup => &upies,
            };

            for (_id, gamepad) in context.gilrs.gamepads() {
                let devpath = gamepad.devpath();
                let minimal_path = &context.event_path_lookup.0.get(devpath);
                if minimal_path.is_none() {
                    continue;
                }
                let minimal_path = minimal_path.unwrap();
                let named_path = context.minimal_path_lookup.0.get(minimal_path);
                if named_path.is_none() {
                    continue;
                }
                let named_path = named_path.unwrap();
                let Some(common_name) : Option<&String> = named_path.common_name.as_ref() else{
                    // NO comomon name ! ok 
                    continue;
                };

                if self.team.players.contains(&common_name) {
                    let (axis_id, scalar) = crate::injoy::snes_namedaxis_to_id_and_scalar(&inaxis);
                    let gilrs_axis = match axis_id {
                        0 => gilrs::Button::DPadRight,
                        1 => gilrs::Button::DPadUp,
                        _ => panic!("Invalid axis_id"),
                    };

                    let value = gamepad.button_data(gilrs_axis);
                    let value = match value {
                        Some(value) => {
                            let vv = value.value();
                            let vvv = match vv {
                                v if v < 0.1 => -1,
                                v if v > 0.9 => 1,
                                _ => 0,
                            } as f32;
                            vvv
                        }
                        None => 0 as f32,
                    };
                    let value = value * scalar.signum();
                    sum += value;
                    count += 1;

                    let letter = Self::inaxis_to_letter(&inaxis, value);

                    let fb_team = match fb_team.as_mut() {
                        Some(fb_team) => fb_team,
                        None => continue,
                    };
                    let mut player = None;
                    for p in fb_team.players.iter_mut() {
                        if &p.player_name == common_name {
                            player = Some(p);
                            break;
                        }
                    }

                    if player.is_none() {
                        continue;
                    }
                    let player = player.unwrap();

                    for f in player.feedback.0.iter_mut() {
                        if clearem.contains(&f.button) {
                            f.state = mjoy_gui::gui::feedback_info::PressState::Unpressed;
                        }
                    }

                    if letter.is_none() {
                        continue;
                    }
                    let letter = letter.unwrap();
                    for f in player.feedback.0.iter_mut() {
                        if f.button == letter {
                            f.state = mjoy_gui::gui::feedback_info::PressState::Pressed;
                        }
                    }
                }
            }

            let average = match count {
                0 => 0 as f32,
                _ => sum / count as f32,
            };
            let average = average.clamp(-1.0f32, 1.0f32);
            let pow = average.abs().powf(2.0f32);
            let average = average.signum() * pow;
            let average_i = (average * 512f32) as i32;
            self.joy.move_axis(out_axis, average_i).unwrap();

            let letter = Self::inaxis_to_letter(&inaxis, average);
            let fb_team = match fb_team.as_mut() {
                Some(fb_team) => fb_team,
                None => continue,
            };

            for f in fb_team.feedback.0.iter_mut() {
                if clearem.contains(&f.button) {
                    f.state = mjoy_gui::gui::feedback_info::PressState::Unpressed;
                }
            }

            if letter.is_none() {
                continue;
            }
            let letter = letter.unwrap();
            for f in fb_team.feedback.0.iter_mut() {
                if f.button == letter {
                    f.state = mjoy_gui::gui::feedback_info::PressState::Pressed;
                }
            }
        }
    }

    fn update_buttons<'b, 'c, 'd, 'e>(&self, context: &'d mut UpdateContext<'b, 'c, 'e>) {
        use crate::injoy::NamedButton;

        let mut fb_team = None;
        for team in context.feedback.teams.iter_mut() {
            if self.team.name == team.team_name {
                fb_team = Some(team);
                break;
            }
        }

        for (i, inbutton) in crate::injoy::NamedButton::iter().enumerate() {
            let mut sum = 0 as f32;
            let mut count = 0;

            let outbutton = inbutton_to_outbutton(&inbutton);

            for (_id, gamepad) in context.gilrs.gamepads() {
                let devpath = gamepad.devpath();
                let minimal_path = &context.event_path_lookup.0.get(devpath);
                if minimal_path.is_none() {
                    continue;
                }
                let minimal_path = minimal_path.unwrap();
                let named_path = context.minimal_path_lookup.0.get(minimal_path);
                if named_path.is_none() {
                    continue;
                }
                let named_path = named_path.unwrap();
                let Some(common_name) = named_path.common_name.as_ref() else
                {
                    // No common name, thats ok
                    continue;
                };

                if context.hat_only_player_names.contains(&common_name) {
                    continue;
                }

                if self.team.players.contains(&common_name) {
                    let button_id: gilrs::Button = crate::injoy::snes_namedbutton_to_id(&inbutton);
                    let value = gamepad.button_data(button_id);
                    let value = match value {
                        Some(value) => {
                            let vv = value.value();
                            let vvv = match vv {
                                v if v < 0.1 => 0,
                                v if v > 0.9 => 1,
                                _ => 0,
                            } as f32;
                            vvv
                        }
                        None => 0 as f32,
                    };

                    sum += value;
                    count += 1;

                    let fb_team = match fb_team.as_mut() {
                        Some(fb_team) => fb_team,
                        None => continue,
                    };
                    let mut player = None;
                    for p in fb_team.players.iter_mut() {
                        if &p.player_name == common_name {
                            player = Some(p);
                            break;
                        }
                    }

                    if player.is_none() {
                        continue;
                    }
                    let player = player.unwrap();

                    let letter = Self::inbutton_to_letter(&inbutton);
                    for f in player.feedback.0.iter_mut() {
                        if f.button == letter {
                            let punp = if value > context.button_threshold {
                                mjoy_gui::gui::feedback_info::PressState::Pressed
                            } else {
                                mjoy_gui::gui::feedback_info::PressState::Unpressed
                            };
                            f.state = punp;
                        }
                    }
                }
            }

            let average = match count {
                0 => 0 as f32,
                _ => sum / count as f32,
            };

            self.joy
                .button_press(outbutton, average > context.button_threshold)
                .unwrap();

            let fb_team = match fb_team.as_mut() {
                Some(fb_team) => fb_team,
                None => continue,
            };

            let letter = Self::inbutton_to_letter(&inbutton);
            for f in fb_team.feedback.0.iter_mut() {
                if f.button == letter {
                    let punp = if average > context.button_threshold {
                        mjoy_gui::gui::feedback_info::PressState::Pressed
                    } else {
                        mjoy_gui::gui::feedback_info::PressState::Unpressed
                    };
                    f.state = punp;
                }
            }
        }
    }

    pub fn update<'b, 'c, 'd, 'e>(&self, context: &'d mut UpdateContext<'b, 'c, 'e>) {
        self.update_axes(context);
        self.update_buttons(context);
        self.joy.synchronise().unwrap();
    }
}

pub struct UpdateContext<'b, 'c, 'e> {
    pub event_path_lookup: &'b joypaths::EventPathLookup,
    pub minimal_path_lookup: &'b joypaths::MinimalPathLookup,
    pub gilrs: &'c mut gilrs::Gilrs,
    pub feedback: &'e mut mjoy_gui::gui::feedback_info::FeedbackInfo,
    pub hat_only_player_names: &'b Vec<String>,
    pub button_threshold: f32,
}

impl Outjoys {
    pub fn new(tl: &TeamLock) -> Self {
        let mut outjoys = Vec::new();
        for team in tl.teams.iter() {
            outjoys.push(Outjoy::new(team.clone(), team.out_index));
        }
        Self { outjoys }
    }

    pub fn overwrite(&mut self, tl: &TeamLock) {
        for (i, team) in tl.teams.iter().enumerate() {
            self.outjoys.get_mut(i).unwrap().mutate_team(team.clone());
        }
    }

    pub fn update<'b, 'c, 'd, 'e>(&self, context: &'d mut UpdateContext<'b, 'c, 'e>) {
        for outjoy in self.outjoys.iter() {
            outjoy.update(context);
        }
    }
}
